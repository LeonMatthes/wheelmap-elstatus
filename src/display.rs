use crate::Equipment;

use chrono::{Datelike, Timelike};
use clap::Args;
use image::{ImageOutputFormat, RgbImage};
use reqwest::blocking::{multipart::Form, Client};
use rgb::ComponentBytes;
use slint::{
    platform::{software_renderer::*, Platform, PlatformError, WindowAdapter},
    Rgb8Pixel, VecModel,
};
use std::{error::Error, io::Cursor, rc::Rc, time::Duration};

const WIDTH: usize = 296;
const HEIGHT: usize = 128;

#[derive(Args, Debug)]
pub struct DisplayArgs {
    /// URL or IP address of the access point.
    #[clap(long)]
    ap_address: String,

    /// MAC of the E-Paper Tag
    #[clap(long)]
    tag: String,
}

struct MyPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for MyPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
}

pub fn update(equipments: &[Equipment], args: &DisplayArgs) -> Result<(), Box<dyn Error>> {
    let image = render_ui(equipments);

    upload_image(args, &image)
}

fn render_ui(equipments: &[Equipment]) -> RgbImage {
    let broken_equipments: Vec<_> = equipments
        .iter()
        .filter(|eq| !eq.working.unwrap_or(false))
        .map(|eq| Elevator {
            name: eq.name.clone().into(),
            place: eq.place.clone().unwrap_or_default().into(),
        })
        .collect();
    let now = chrono::Local::now();
    let day = now.day();
    let month = now.month();
    let hour = now.hour();
    let minute = now.minute();

    let last_update = format!("{day}.{month}. - {hour}:{minute}");

    let mut frame_buffer = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];

    let platform = Box::new(MyPlatform {
        window: MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer),
    });

    let window = Rc::clone(&platform.window);
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));

    slint::platform::set_platform(platform).unwrap();

    let main_tag = MainTag::new().unwrap();
    main_tag.set_broken(Rc::new(VecModel::from(broken_equipments)).into());
    main_tag.set_last_update(last_update.into());
    main_tag.show().unwrap();

    slint::platform::update_timers_and_animations();

    window.draw_if_needed(|software_renderer| {
        println!("rendering!");
        software_renderer.render(&mut frame_buffer, WIDTH);
    });

    let frame_buffer = Vec::from(frame_buffer.as_bytes());
    let image = image::RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, frame_buffer).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .open("elstatus.jpg")
        .unwrap();
    image
        .write_to(&mut file, ImageOutputFormat::Jpeg(100))
        .unwrap();
    image
}

fn try_uploading(args: &DisplayArgs, client: &Client) -> Result<(), Box<dyn Error>> {
    let form = Form::new()
        .text("mac", args.tag.clone())
        .text("dither", "0")
        .file("elstatus.jpg", "elstatus.jpg")?;
    let request = client
        .post(format!("http://{}/imgupload", args.ap_address))
        .multipart(form)
        .send()?;

    request.error_for_status()?;
    Ok(())
}

fn upload_image(args: &DisplayArgs, image: &RgbImage) -> Result<(), Box<dyn Error>> {
    let mut cursor = Cursor::new(Vec::new());
    image.write_to(&mut cursor, image::ImageOutputFormat::Jpeg(100))?;

    let client = Client::new();

    let mut delay = Duration::from_millis(100);
    let mut last_result = Ok(());
    const NUM_RETRIES: i32 = 5;
    for i in 1..NUM_RETRIES + 1 {
        println!("Uploading");
        let result = try_uploading(&args, &client);
        if result.is_ok() {
            return Ok(());
        }

        last_result = result;
        println!(
            "{i}/{NUM_RETRIES} upload failed - retrying in {} ms!",
            delay.as_millis()
        );
        std::thread::sleep(delay);
        delay *= 2;
    }

    last_result
}

slint::include_modules!();
