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
use std::{error::Error, path::Path, rc::Rc, sync::Mutex, time::Duration};

const WIDTH: usize = 296;
const HEIGHT: usize = 128;

#[derive(Args, Debug)]
pub struct DisplayArgs {
    /// URL or IP address of the access point.
    #[clap(long)]
    ap_address: String,

    /// MAC of the E-Paper Tag
    #[clap(long)]
    main_tag: String,

    /// MAC of the secondary E-Paper Tag
    #[clap(long)]
    secondary_tag: String,
}

struct MyPlatform {
    main_tag: Rc<MinimalSoftwareWindow>,
    secondary_tag: Rc<MinimalSoftwareWindow>,
    index: Mutex<i32>,
}

impl Platform for MyPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let mut index = self.index.lock().unwrap();
        let current = *index;
        *index += 1;
        match current {
            0 => Ok(self.main_tag.clone()),
            1 => Ok(self.secondary_tag.clone()),
            _ => Err(PlatformError::Other("No more available tags!".to_owned())),
        }
    }
}

pub fn update(equipments: &[Equipment], args: &DisplayArgs) -> Result<(), Box<dyn Error>> {
    render_ui(equipments);

    upload_image(&args.ap_address, &args.main_tag, "elstatus.jpg")?;
    println!("⏳ Waiting 10 seconds before uploading secondary image");
    std::thread::sleep(Duration::from_secs(10));
    upload_image(
        &args.ap_address,
        &args.secondary_tag,
        "elstatus_secondary.jpg",
    )
}

fn write_frame_buffer_to<P: AsRef<Path>>(path: P, frame_buffer: &[Rgb8Pixel]) -> RgbImage {
    let frame_buffer = Vec::from(frame_buffer.as_bytes());
    let image = image::RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, frame_buffer).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .open(path)
        .unwrap();
    image
        .write_to(&mut file, ImageOutputFormat::Jpeg(100))
        .unwrap();
    image
}

fn render_ui(equipments: &[Equipment]) -> (RgbImage, RgbImage) {
    println!("💻 Rendering GUI");
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

    let mut main_tag_fb = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
    let mut secondary_tag_fb = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];

    let platform = Box::new(MyPlatform {
        main_tag: MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer),
        secondary_tag: MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer),
        index: Mutex::new(0),
    });

    let main_window = Rc::clone(&platform.main_tag);
    main_window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    let secondary_window = Rc::clone(&platform.secondary_tag);
    secondary_window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));

    slint::platform::set_platform(platform).unwrap();

    let main_tag = ElStatus::new().unwrap();
    let vec_model = Rc::new(VecModel::from(broken_equipments));
    main_tag.set_broken(Rc::clone(&vec_model).into());
    main_tag.set_last_update(last_update.clone().into());
    main_tag.set_main(true);
    main_tag.show().unwrap();

    let secondary_tag = ElStatus::new().unwrap();
    secondary_tag.set_broken(vec_model.into());
    secondary_tag.set_last_update(last_update.into());
    secondary_tag.set_main(false);
    secondary_tag.show().unwrap();

    slint::platform::update_timers_and_animations();

    for (window, fb) in [
        (main_window, &mut main_tag_fb),
        (secondary_window, &mut secondary_tag_fb),
    ] {
        window.draw_if_needed(|software_renderer| {
            software_renderer.render(fb, WIDTH);
        });
    }

    let main_image = write_frame_buffer_to("elstatus.jpg", &main_tag_fb);
    let secondary_image = write_frame_buffer_to("elstatus_secondary.jpg", &secondary_tag_fb);

    (main_image, secondary_image)
}

fn try_uploading(
    ap_address: &str,
    tag_mac: &str,
    client: &Client,
    image_path: &str,
) -> Result<(), Box<dyn Error>> {
    let form = Form::new()
        .text("mac", tag_mac.to_owned())
        .text("dither", "0")
        .file(image_path.to_owned(), image_path)?;
    let request = client
        .post(format!("http://{}/imgupload", ap_address))
        .multipart(form)
        .send()?;

    request.error_for_status()?;
    Ok(())
}

fn upload_image(ap_address: &str, tag_mac: &str, image_path: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    let mut delay = Duration::from_millis(100);
    let mut last_result = Ok(());
    const NUM_RETRIES: i32 = 5;
    for i in 1..NUM_RETRIES + 1 {
        println!("📶 Uploading");
        let result = try_uploading(ap_address, tag_mac, &client, image_path);
        if result.is_ok() {
            println!("✅ Successfully uploaded");
            return Ok(());
        }

        last_result = result;
        println!(
            "⚠️ {i}/{NUM_RETRIES} upload failed - ⏳ retrying in {} ms!",
            delay.as_millis()
        );
        std::thread::sleep(delay);
        delay *= 2;
    }

    last_result
}

slint::include_modules!();
