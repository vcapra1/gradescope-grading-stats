use std::{thread, sync::{Mutex, Arc}, process::exit, time::{Instant, Duration}};
use actix_web::{get, post, web, App, HttpResponse, HttpRequest, HttpServer, Responder};
use crate::gradescope::{client::{GradescopeClient, ClientError}, stats::Stats};

struct AppState {
    stats: Arc<Mutex<Option<Stats>>>,
}

async fn get(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Some(stats) = &*data.stats.lock().unwrap() {
        format!("{:?}", stats)
    } else {
        "no data".to_string()
    }
}

pub fn start(email: String, password: String, port: u16, interval: u32, courseid: u64, assignmentid: u64) {
    let stats = Arc::new(Mutex::new(None));

    let data = web::Data::new(AppState {
        stats: Arc::clone(&stats),
    });

    let client = {
        /* Create a Gradescope client */
        let mut client = match GradescopeClient::new(None) {
            Ok(client) => client,
            Err(_) => {
                eprintln!("Error connecting to Gradescope");
                exit(1);
            }
        };

        /* Authenticate */
        match client.login(email, password) {
            Err(ClientError::InvalidLogin) => {
                eprintln!("Invalid credentials");
                exit(1);
            }
            Err(_) => {
                eprintln!("There was an error authenticating");
                exit(1);
            }
            Ok(_) => ()
        }

        client
    };

    thread::spawn(move || {
        let mut timer = Instant::now();
        let interval = Duration::from_secs(interval as u64 * 60);
        let sleep_duration = Duration::from_secs(10);
        let mut s = client.fetch_grading_stats(courseid, assignmentid).unwrap();
        stats.lock().unwrap().replace(s.clone());

        loop {
            if timer.elapsed() >= interval {
                timer = Instant::now();
                client.refresh_grading_stats(&mut s);
                stats.lock().unwrap().replace(s.clone());
            } else {
                thread::sleep(sleep_duration);
            }
        }
    });

    actix_web::rt::System::new("main").block_on(async move {
        HttpServer::new(move || {
            App::new()
                .app_data(data.clone())
                .route("/", web::get().to(get))
        }).bind(format!("0.0.0.0:{}", port)).unwrap().run().await.unwrap();
    });
}
