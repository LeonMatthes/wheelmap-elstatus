use serde_json::Value;
use std::error::Error;

const ACCESS_TOKEN: &str = "27be4b5216aced82122d7cf8f69e4a07";

const EQUIPMENTS: [&str; 1] = ["DibiBDQsW5XcyxmqW"];

#[derive(Debug)]
enum EquipmentAccessError {
    MissingValue(String),
}

impl std::fmt::Display for EquipmentAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipmentAccessError::MissingValue(value) => write!(f, "Missing JSON value: {}", value),
        }
    }
}

impl Error for EquipmentAccessError {}

#[derive(Debug)]
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

    let json: Value = serde_json::from_str(&*request.text()?)?;

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
        )))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let equipments: Vec<_> = EQUIPMENTS
        .into_iter()
        .map(get_equipment)
        .filter_map(Result::ok)
        .collect();

    let num_ok = equipments
        .iter()
        .filter(|eq| eq.working.unwrap_or_default())
        .count();

    let num_failed = equipments
        .iter()
        .filter(|eq| !eq.working.unwrap_or(true))
        .count();

    let num_unknown = equipments.iter().filter(|eq| !eq.working.is_none()).count();

    println!("{:?}", get_equipment()?);
    Ok(())
}
