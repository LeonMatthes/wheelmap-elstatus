use clap::{Parser, Subcommand};
use elstatus::*;
use std::{error::Error, path::PathBuf};
use tera::Tera;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, value_name = "FILE_PATH")]
    elevator_list: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Send the status via E-Mail
    EMail(email::EMailArgs),
    /// Update the epaper displays using OpenEPaperLink
    Display(display::DisplayArgs),
}

fn read_equipment_list(cli: &Cli) -> (Vec<Equipment>, Vec<Box<dyn Error>>) {
    let json = std::fs::read_to_string(
        cli.elevator_list
            .clone()
            .unwrap_or_else(|| "./equipments.json".into()),
    );
    let json = match json {
        Ok(json) => json,
        Err(err) => {
            return (vec![], vec![Box::new(err)]);
        }
    };
    let equipment_list: Result<Vec<EquipmentList>, _> = serde_json::from_str(&json);
    let equipment_list = match equipment_list {
        Ok(equipment_list) => equipment_list,
        Err(err) => {
            return (vec![], vec![Box::new(err)]);
        }
    };
    let (equipments, errors): (Vec<_>, Vec<_>) = equipment_list
        .iter()
        .map(get_equipments)
        .partition(Result::is_ok);

    let equipments: Vec<_> = equipments.into_iter().flat_map(Result::unwrap).collect();
    let errors: Vec<_> = errors
        .into_iter()
        .map(Result::err)
        .map(Option::unwrap)
        .collect();

    (equipments, errors)
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let (equipments, errors) = read_equipment_list(&cli);

    for equipment in equipments.iter() {
        println!("{:?}", equipment);
    }

    for error in errors.iter() {
        println!("Error: {}", error);
    }

    match cli.command {
        Command::EMail(email_args) => {
            let mut tera = Tera::default();
            tera.add_raw_templates(vec![
                ("status.html", include_str!("templates/status.html")),
                ("status.txt", include_str!("templates/status.txt")),
                ("errors.txt", include_str!("templates/errors.txt")),
            ])?;

            email::send_result(&equipments, &errors, &tera, &email_args);

            email::send_errors(&errors, &tera, &email_args);
            Ok(())
        }
        Command::Display(display_args) => display::update(&equipments, &display_args),
    }
}
