use std::{
    collections::HashMap,
    fs::File,
    sync::{Arc, Mutex},
};
use tide::prelude::*;
use tide::Request;

#[derive(serde::Serialize, serde::Deserialize)]
struct QueryData {
    json: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Access {
    User,
    Admin,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Person {
    name: String,
    santa_to: String,
    access: Access,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Group {
    name: String,
    people: Vec<Person>,
    closed: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DataBase {
    groups: HashMap<i8, Group>,
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    let database = match File::open("data.base") {
        Ok(file) => serde_json::from_reader(file).map_err(|err| {
            let err = std::io::Error::from(err);
            std::io::Error::new(
                err.kind(),
                format!("Failed to read from database file. {err}"),
            )
        })?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Database file not found. Creating one");

            let file = File::create("data.base").map_err(|err| {
                std::io::Error::new(err.kind(), format!("Failed to create database file. {err}"))
            })?;
            let database = DataBase {
                groups: HashMap::new(),
            };
            serde_json::to_writer(file, &database).map_err(|err| {
                let err = std::io::Error::from(err);
                std::io::Error::new(
                    err.kind(),
                    format!("Failed to write to database file. {err}"),
                )
            })?;

            database
        }
        Err(err) => {
            panic!("Failed to open database file. {err}");
        }
    };

    let state = Arc::new(Mutex::new(database));
    let mut app = tide::with_state(state);

    app.at("/").post(index);
    app.at("/groups/list").get(get_groups);
    app.at("/groups/create").post(create_group);
    app.at("/groups/join").post(join_group);
    app.at("/groups/members").post(get_members);
    app.at("/terminate")
        .get(|request: tide::Request<Arc<Mutex<DataBase>>>| async move {
            let state = request.state();
            std::fs::write(
                "data.base",
                serde_json::to_string(state).unwrap().as_bytes(),
            )?;
            std::process::exit(0);
            #[allow(unreachable_code)]
            Ok("done")
        });
    app.listen("192.168.0.103:8080").await?;

    println!("Done");
    Ok(())
}

// fn is_group_exist(groups: &HashMap<i8, Group>, group_name: &String) -> bool {
//     groups.iter().any(|i| i.1.name.eq(group_name))
// }

/*
200 - Ok
400 - Bad Request
403 - Forbidden
405 - Method Not Allowed
*/
fn returnable_value(text: &str, is_json: bool, code: usize) -> tide::Result {
    if is_json {
        return Ok(json!({
            "code": code,
            "message": text
        })
        .into());
    }
    Ok(text.into())
}

fn is_person_exist(groups: &HashMap<i8, Group>, name: &String) -> bool {
    groups
        .iter()
        .any(|i| i.1.people.iter().any(|j| j.name.eq(name)))
}

async fn join_group(mut req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Data {
        name: String,
        group_name: String,
    }
    let data: Data = req.body_json().await.unwrap_or(Data {
        name: String::new(),
        group_name: String::new(),
    });

    let QueryData { json } = req.query().unwrap_or(QueryData { json: false });

    if data.name == "" || data.group_name == "" {
        return returnable_value("Bad data", json, 400);
    }

    let state = req.state();
    let mut guard = state.lock().unwrap();

    if is_person_exist(&guard.groups, &data.name) {
        return returnable_value("You have to leave from group to join other", json, 405);
    }

    let mut groups = guard.groups.iter_mut();

    match groups.find(|i| i.1.name == data.group_name) {
        None => {
            return returnable_value("Group with that name does not exist", json, 400);
        }
        Some(i) => {
            if i.1.closed {
                return returnable_value("This group is closed!", json, 403);
            }
            let new_person = Person {
                name: data.name,
                santa_to: String::new(),
                access: Access::User,
            };
            i.1.people.push(new_person);
        }
    }

    returnable_value(
        format!("Done! You are in group \"{}\" now", data.group_name).as_str(),
        json,
        200,
    )
}

async fn create_group(mut req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Data {
        name: String,
        group_name: String,
    }
    let data: Data = req.body_json().await.unwrap_or(Data {
        name: String::new(),
        group_name: String::new(),
    });

    let QueryData { json } = req.query().unwrap_or(QueryData { json: false });

    if data.name == "" || data.group_name == "" {
        return returnable_value("Bad data", json, 400);
    }

    let state = req.state();
    let mut guard = state.lock().unwrap();
    let mut groups = guard.groups.iter();

    if is_person_exist(&guard.groups, &data.name) {
        return returnable_value("You have to leave from group to create other", json, 405);
    }

    match groups.find(|i| i.1.name == data.group_name) {
        None => {
            let new_group_id: i8 = guard.groups.len() as i8;
            let new_admin = Person {
                name: data.name,
                santa_to: String::new(),
                access: Access::Admin,
            };
            let new_group = Group {
                name: data.group_name,
                people: vec![new_admin],
                closed: false,
            };
            guard.groups.insert(new_group_id, new_group);
        }
        Some(_) => {
            return returnable_value("Group with this name is exist", json, 400);
        }
    }

    returnable_value("Group is created", json, 200)
}

async fn get_members(mut req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Data {
        name: String,
        group_name: String,
    }
    let data: Data = req.body_json().await.unwrap_or(Data {
        name: String::new(),
        group_name: String::new(),
    });

    let QueryData { json } = req.query().unwrap_or(QueryData { json: false });

    if data.name == "" || data.group_name == "" {
        return returnable_value("Bad data", json, 400);
    }

    let state = req.state();
    let guard = state.lock().unwrap();
    let mut groups = guard.groups.iter();
    let mut out_message: String = String::new();

    match groups.find(|i| i.1.name == data.group_name) {
        Some(g) => {
            if json {
                return Ok(json!({
                    "code": 200,
                    "message": {
                        "group_name": data.group_name,
                        "people": g.1.people
                    }
                })
                .into());
            } else {
                let mut id: i8 = 0;
                for person in &g.1.people {
                    out_message += format!(
                        "{}. Name: {}. Access: {:?}\n",
                        id,
                        person.name.as_str(),
                        person.access
                    )
                    .as_str();
                    id += 1;
                }
            }
        }
        None => {
            return returnable_value("There is not group with that name", json, 400);
        }
    }

    Ok(out_message.into())
}

async fn get_groups(req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    let state = req.state();

    let QueryData { json } = req.query().unwrap_or(QueryData { json: false });

    let guard = state.lock().unwrap();
    let groups = guard.groups.iter();
    let mut out_message: String = String::new();

    if guard.groups.len() == 0 {
        return returnable_value("There is no any group", json, 200);
    } else {
        if json {
            out_message = serde_json::to_string(state).unwrap();
        } else {
            out_message += "Groups: \n";
            for (id, group) in groups {
                out_message += format!(
                    "Id: {}. Group name: \"{}\". Persons: {}. Is closed: {}\n",
                    id,
                    group.name,
                    group.people.len(),
                    group.closed
                )
                .as_str();
            }
        }
    }

    Ok(out_message.into())
}

async fn index(mut req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct User {
        name: String,
    }
    let user: User = req.body_json().await.unwrap_or(User {
        name: String::new(),
    });

    let QueryData { json } = req.query().unwrap_or(QueryData { json: false });

    if user.name == "" {
        return returnable_value("Who are you?", json, 200);
    }

    returnable_value(format!("Hello {}!", user.name).as_str(), json, 200)
}
