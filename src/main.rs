use clap::{Parser, Subcommand};
use elstatus::*;
use lazy_static::lazy_static;
use std::error::Error;
use tera::Tera;

lazy_static! {
    static ref EQUIPMENTS: Vec<EquipmentList> = vec! [
        // Berlin-Wannsee
        EquipmentList {
            latitude: 52.422206,
            longitude: 13.1810253,
            equipment_searches: vec![
                "Gleis 1/2",
                "Gleis 3/4",
                "Ausgang Vorplatz",
            ]
        },
        // Potsdam Griebnitzsee
        EquipmentList {
            latitude: 52.394356,
            longitude: 13.127521,
            equipment_searches: vec![
                "Gleis 1/2",
                "Gleis 5"
            ]
        },
        // Berlin Hauptbahnhof
        EquipmentList {
            latitude: 52.525303,
            longitude: 13.369338,
            equipment_searches: vec![
                "Gleis 3/4 (C/D) - Gleis 15/16",
                "Gleis 5/6 (C/D) - Gleis 15/16",
            ]
        },
        // Zoologischer Garten
        EquipmentList {
            latitude: 52.507_28,
            longitude: 13.332334,
            equipment_searches: vec![
                "Gleis 5/6 (S-Bahn)"
            ]
        },
        // Potsdamer Platz
        EquipmentList {
            latitude: 52.50925,
            longitude: 13.3766,
            equipment_searches: vec![
                "Gleis 11/12 (S-Bahn)",
                "Gleis 13/14 (S-Bahn)",
                "EG zu UG"
            ]
        },
        // Anhalter Bahnhof
        EquipmentList {
            latitude: 52.503283,
            longitude: 13.38133,
            equipment_searches: vec! [
                "Gleis 1/2 (S-Bahn)",
                "Gleis 3/4 (S-Bahn)",
            ]
        }
    ];
}

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

    let (equipments, errors): (Vec<_>, Vec<_>) = EQUIPMENTS
        .iter()
        .map(get_equipments)
        .partition(Result::is_ok);

    let equipments: Vec<_> = equipments.into_iter().flat_map(Result::unwrap).collect();
    let errors: Vec<_> = errors
        .into_iter()
        .map(Result::err)
        .map(Option::unwrap)
        .collect();

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
