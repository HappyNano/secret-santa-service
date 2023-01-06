use std::{
    collections::HashMap,
    fs::File,
    sync::{Arc, Mutex},
};
use tide::Request;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Access {
    User,
    Admin,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Person {
    name: String,
    in_group: i8,
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

    if data.name == "" || data.group_name == "" {
        return Ok("Bad data".into());
    }

    let state = req.state();
    let mut guard = state.lock().unwrap();

    if is_person_exist(&guard.groups, &data.name) {
        return Ok("You have to leave from group to join other".into());
    }

    let mut groups = guard.groups.iter_mut();

    match groups.find(|i| i.1.name == data.group_name) {
        None => {
            return Ok("Group with that name does not exist".into());
        }
        Some(i) => {
            if i.1.closed {
                return Ok("This group is closed!".into());
            }
            let new_person = Person {
                name: data.name,
                in_group: i.0.clone(),
                santa_to: String::new(),
                access: Access::User,
            };
            i.1.people.push(new_person);
        }
    }

    Ok(format!("You are in group \"{}\" now", data.group_name).into())
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

    if data.name == "" || data.group_name == "" {
        return Ok("Bad data".into());
    }

    let state = req.state();
    let mut guard = state.lock().unwrap();
    let mut groups = guard.groups.iter();

    if is_person_exist(&guard.groups, &data.name) {
        return Ok("You have to leave from group to create other".into());
    }

    match groups.find(|i| i.1.name == data.group_name) {
        None => {
            let new_group_id: i8 = guard.groups.len() as i8;
            let new_admin = Person {
                name: data.name,
                in_group: new_group_id,
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
            return Ok("Group with this name is exist".into());
        }
    }

    Ok("Group is created".into())
}

async fn get_groups(req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    let state = req.state();
    let guard = state.lock().unwrap();
    let groups = guard.groups.iter();
    let mut out_message: String = String::new();

    if guard.groups.len() == 0 {
        out_message += "There is no group";
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

    if user.name == "" {
        return Ok(format!("Who are you?").into());
    }

    Ok(format!("Hello {}!", user.name).into())
}
