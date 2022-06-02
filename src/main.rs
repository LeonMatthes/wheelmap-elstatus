use serde::Serialize;
use serde_json::Value;
use std::error::Error;

const ACCESS_TOKEN: &str = "aacf2356050724fdb65b7351f5f01eef";

const EQUIPMENTS: [&str; 5] = [
    "qo7jxc6E524sBuCp7",
    "DibiBDQsW5XcyxmqW",
    "6wE7kq4Ls8w9u6rQ2",
    "Rg5vjFd8aYChyc2uX",
    "99LqXyQugBDKs5myG",
];

#[derive(Debug)]
enum EquipmentAccessError {
    MissingValue(String, String),
}

impl std::fmt::Display for EquipmentAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipmentAccessError::MissingValue(value, json) => {
                write!(f, "Missing value: {} in JSON: {}", value, json)
            }
        }
    }
}

impl Error for EquipmentAccessError {}

#[derive(Debug, Serialize)]
struct Equipment {
    name: String,
    working: Option<bool>,
    place: Option<String>,
}

fn get_equipment(equipment: &str) -> Result<Equipment, Box<dyn Error>> {
    let request = reqwest::blocking::get(format!(
        "https://accessibility-cloud.freetls.fastly.net/equipment-infos/{}.json?appToken={}",
        equipment, ACCESS_TOKEN
    ))?;

    let json_string = request.text()?;
    let json: Value = serde_json::from_str(&*json_string)?;

    if let Some(properties) = &json.get("properties") {
        let working = properties
            .get("isWorking")
            .unwrap_or(&Value::default())
            .as_bool();
        let name = properties
            .get("description")
            .and_then(|description| description.get("de").unwrap_or(description).as_str())
            .unwrap_or(&*format!("'Unknown name! - Id: {}'", equipment))
            .to_owned();
        let place = properties
            .get("placeInfoName")
            .and_then(Value::as_str)
            .map(str::to_owned);

        Ok(Equipment {
            name,
            working,
            place,
        })
    } else {
        Err(Box::new(EquipmentAccessError::MissingValue(
            "properties".to_owned(),
            json_string,
        )))
    }
}

use lettre::transport::smtp::authentication::Credentials;
use lettre::{message::MultiPart, Message, SmtpTransport, Transport};

use tera::Tera;

fn send_result(
    equipments: &Vec<Equipment>,
    errors: &Vec<Box<dyn Error>>,
    tera: &Tera,
    args: &Args,
) {
    let num_ok = equipments
        .iter()
        .filter(|eq| eq.working.unwrap_or_default())
        .count();

    let num_failed = equipments
        .iter()
        .filter(|eq| !eq.working.unwrap_or(true))
        .count();

    let num_unknown = equipments.iter().filter(|eq| eq.working.is_none()).count() + errors.len();

    let ok_status = if num_ok > 0 { "✅" } else { "" };
    let failed_status = if num_failed > 0 { "⛔" } else { "" };
    let unknown_status = if num_unknown > 0 { "❔" } else { "" };

    let status_message = match (num_failed, num_ok, num_unknown) {
        (0, _, _) if num_ok > 0 && num_unknown > 0 => "Kein defekter Aufzug (einige Unbekannt)!",
        (0, 0, _) => "Warnung: Aufzugstatus unbekannt!",
        (0, _, 0) => "Alle Aufzüge funktionieren!",
        (_, _, _) => "Achtung: Defekter Aufzug auf Arbeitsweg!",
    };

    let mut context = tera::Context::new();
    context.insert("equipments", equipments);
    context.insert(
        "errors",
        &errors.iter().map(|err| err.to_string()).collect::<Vec<_>>(),
    );
    let html_message = tera
        .render("status.html", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err.to_string()));
    let text_message = tera
        .render("status.txt", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err.to_string()));

    let email = Message::builder()
        .from(format!("ElStatus <{}>", args.smtp_user).parse().unwrap())
        .to(args.status_address.parse().unwrap())
        .subject(format!(
            "{}{}{} {}",
            failed_status, ok_status, unknown_status, status_message
        ))
        .multipart(MultiPart::alternative_plain_html(
            text_message,
            html_message,
        ))
        .unwrap();

    let creds = Credentials::new(args.smtp_user.clone(), args.smtp_password.clone());

    let mailer = SmtpTransport::relay(&*args.smtp_server)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Status email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}

fn send_errors(errors: &Vec<Box<dyn Error>>, tera: &Tera, args: &Args) {
    if errors.is_empty() {
        return;
    }

    let mut context = tera::Context::new();
    context.insert(
        "errors",
        &errors.iter().map(|err| err.to_string()).collect::<Vec<_>>(),
    );
    let text_message = tera
        .render("errors.txt", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err.to_string()));

    let email = Message::builder()
        .from(format!("ElStatus <{}>", args.smtp_user).parse().unwrap())
        .to(args.errors_address.parse().unwrap())
        .subject(format!(
            "{} Errors encountered when checking elevator status",
            errors.len()
        ))
        .body(text_message)
        .unwrap();

    let creds = Credentials::new(args.smtp_user.clone(), args.smtp_password.clone());

    let mailer = SmtpTransport::relay(&*args.smtp_server)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Errors E-Mail sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// E-Mail Address to send the status to
    #[clap(long)]
    status_address: String,

    /// E-Mail Address to send errors to (if any)
    #[clap(long)]
    errors_address: String,

    /// smtp server address
    #[clap(long)]
    smtp_server: String,

    /// smtp username
    #[clap(long)]
    smtp_user: String,

    /// smtp password
    #[clap(long)]
    smtp_password: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut tera = Tera::default();
    tera.add_raw_templates(vec![
        ("status.html", include_str!("templates/status.html")),
        ("status.txt", include_str!("templates/status.txt")),
        ("errors.txt", include_str!("templates/errors.txt")),
    ])?;

    let (equipments, errors): (Vec<_>, Vec<_>) = EQUIPMENTS
        .into_iter()
        .map(get_equipment)
        .partition(Result::is_ok);

    let equipments: Vec<_> = equipments.into_iter().map(Result::unwrap).collect();
    let errors: Vec<_> = errors
        .into_iter()
        .map(Result::err)
        .map(Option::unwrap)
        .collect();

    for equipment in equipments.iter() {
        println!("{:?}", equipment);
    }

    send_result(&equipments, &errors, &tera, &args);

    send_errors(&errors, &tera, &args);

    Ok(())
}
