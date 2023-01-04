use std::{
    collections::HashMap,
    fs::File,
    sync::{Arc, Mutex},
};
use tide::{Request, new};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Access {
    Guest,
    User,
    Admin,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Person {
    name: String,
    santa_to: String,
    in_group: i8,
    access: Access,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Group {
    people: Vec<Person>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DataBase {
    users: HashMap<i8, Person>,
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
                users: HashMap::new(),
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

    app.with(tide::sessions::SessionMiddleware::new(
        tide::sessions::MemoryStore::new(),
        "we recommend you use std::env::var(\"TIDE_SECRET\").unwrap().as_bytes() instead of a fixed value".as_bytes()
        ));

    app.with(tide::utils::Before(
        |mut request: tide::Request<Arc<Mutex<DataBase>>>| async move {
            let session = request.session_mut();
            let visits: usize = session.get("visits").unwrap_or_default();
            let user_id: i8 = session.get("user_id").unwrap_or(-1);
            if user_id == -1 {
                session.insert("user_id", -1).unwrap();
            }
            session.insert("visits", visits + 1).unwrap();
            request
        },
    ));

    // app.at("/").get(|req: tide::Request<Arc<Mutex<DataBase>>>| async move {
    //     let visits: usize = req.session().get("visits").unwrap();
    //     Ok(format!("you have visited this website {} times", visits))
    // });

    app.at("/reset").get(quit);
    app.at("/login").post(login);

    app.at("/").get(index);
    app.at("/terminate").get(|request: tide::Request<Arc<Mutex<DataBase>>>| async move {
        let state = request.state();
        std::fs::write("data.base", serde_json::to_string(state).unwrap().as_bytes())?;
        std::process::exit(0);
        #[allow(unreachable_code)]
        Ok("done")
    });
    app.listen("192.168.0.103:8080").await?; 

    
    println!("Done");
    Ok(())
}

async fn login(mut req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct TmpPerson {name: String}
    let tmp_person: TmpPerson = req.body_json().await?;

    if tmp_person.name == "" {
        return Ok(tide::Redirect::new("/").into());
    }

    let user_id: i8;
    
    {
        let state = req.state();
        let mut guard = state.lock().unwrap();
        match guard.users.iter().find(
            |p| p.1.name == tmp_person.name
        ) {
            None => {
                let person = Person {
                    name: tmp_person.name,
                    santa_to: String::new(),
                    in_group: -1,
                    access: Access::User
                }; 
                let new_id: i8 = guard.users.len() as i8; 
                guard.users.insert(new_id, person);
                user_id = new_id;
            }
            Some(p) => {
                user_id = p.0.clone();
            },
        };
    }
    
    let session = req.session_mut();
    session.insert("user_id", user_id)?;
    
    Ok(tide::Redirect::new("/").into())
}

async fn index(req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    let user_id: i8 = req.session().get("user_id").unwrap();
    if user_id == -1
    {
        return Ok("You have to log in using POST request on /login with your name".into());
    }
    let state = req.state();
    let guard = state.lock().unwrap();
    let user = guard.users.iter().find(
        |p| {
            p.0 == &user_id
        });
    let user_name = &user.unwrap().1.name;
    Ok(format!("Hello {user_name}!").into())
}

async fn quit(mut req: Request<Arc<Mutex<DataBase>>>) -> tide::Result {
    req.session_mut().destroy();
    Ok(tide::Redirect::new("/").into())
}
