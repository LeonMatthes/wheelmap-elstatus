use std::error::Error;

use clap::{Parser, Subcommand};
use elstatus::{display, email, Equipment};
use tera::Tera;

static EQUIPMENT_JSON: &str = include_str!("elstatus.broken.json");

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Send the status via E-Mail
    EMail(email::EMailArgs),
    /// Update the epaper displays using OpenEPaperLink
    Display(display::DisplayArgs),
}
fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let equipments: Vec<Equipment> = serde_json::from_str(EQUIPMENT_JSON)?;

    for equipment in equipments.iter() {
        println!("{:?}", equipment);
    }

    match cli.command {
        Command::EMail(email_args) => {
            let mut tera = Tera::default();
            tera.add_raw_templates(vec![
                ("status.html", include_str!("../src/templates/status.html")),
                ("status.txt", include_str!("../src/templates/status.txt")),
                ("errors.txt", include_str!("../src/templates/errors.txt")),
            ])?;

            email::send_result(&equipments, &vec![], &tera, &email_args);

            email::send_errors(&vec![], &tera, &email_args);
            Ok(())
        }
        Command::Display(display_args) => display::update(&equipments, &display_args),
    }
}

