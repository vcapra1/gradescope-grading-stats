extern crate reqwest;
extern crate scraper;
extern crate rpassword;
extern crate clap;

mod gradescope;
mod server;

use std::process::exit;
use clap::{App, Arg};
use gradescope::client::{GradescopeClient, ClientError};

fn main() {
    let args = App::new("Gradescope Grading Statistics")
        .version("1.0")
        .author("Vinnie Caprarola <vinnie@vcaprarola.me>")
        .arg(Arg::with_name("email")
             .short("u")
             .long("email")
             .value_name("EMAIL")
             .help("Specify the email you use to login to Gradescope")
             .takes_value(true)
             .required(true))
        .arg(Arg::with_name("password")
             .short("P")
             .long("password")
             .value_name("PASSWORD")
             .help("Specify the password you use to login to Gradescope (it is recommended that you do not supply this as an argument, but the option is here for scripting purposes)")
             .takes_value(true))
        .arg(Arg::with_name("course-id")
             .short("c")
             .long("course-id")
             .value_name("ID")
             .help("Gradescope course ID")
             .takes_value(true)
             .required(true))
        .arg(Arg::with_name("assignment-id")
             .short("a")
             .long("assignment-id")
             .value_name("ID")
             .help("Gradescopes assignment ID")
             .takes_value(true)
             .required(true))
        .arg(Arg::with_name("server")
             .short("s")
             .long("server")
             .help("Run the program in server mode")
             .requires("port")
             .requires("interval"))
        .arg(Arg::with_name("port")
             .short("p")
             .long("port")
             .help("The port to run the server on")
             .takes_value(true)
             .requires("server"))
        .arg(Arg::with_name("interval")
             .short("i")
             .long("interval")
             .value_name("MINUTES")
             .help("The refresh interval, in minutes")
             .takes_value(true)
             .requires("server"))
        .get_matches();

    /* Get the user's email */
    let email = args.value_of("email").unwrap().into();
    
    /* Get the user's password */
    let password = if let Some(password) = args.value_of("password") {
        password.into()
    } else {
        match rpassword::prompt_password_stderr("Enter your Gradescope password: ") {
            Ok(password) => {
                eprintln!("");
                password
            }
            Err(_) => {
                eprintln!("Error reading password");
                exit(1);
            }
        }
    };

    /* Get course id */
    let courseid = match args.value_of("course-id").unwrap().parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            eprintln!("Invalid course ID");
            exit(1);
        }
    };

    /* Get assignment id */
    let assignmentid = match args.value_of("assignment-id").unwrap().parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            eprintln!("Invalid assignment ID");
            exit(1);
        }
    };

    /* Check if server mode */
    if args.is_present("server") {
        /* Get the port to listen on */
        let port = match args.value_of("port").unwrap().parse::<u32>() {
            Ok(port) => {
                if port < 65536 {
                    port as u16
                } else {
                    eprintln!("Invalid port specified");
                    exit(1);
                }
            }
            Err(_) => {
                eprintln!("Invalid port specified");
                exit(1);
            }
        };
        
        /* Get the refresh interval */
        let interval = match args.value_of("interval").unwrap().parse::<u32>() {
            Ok(interval) => interval,
            Err(_) => {
                eprintln!("Invalid interval specified");
                exit(1);
            }
        };

        /* Start the server */
        server::start(email, password, port, interval, courseid, assignmentid);
    } else {
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

        /* Fetch grading stats */
        println!("{:?}", client.fetch_grading_stats(courseid, assignmentid).unwrap());
    }
}
