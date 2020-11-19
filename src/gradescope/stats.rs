use std::{fmt, collections::HashMap};
use super::client::ClientError;
use reqwest::blocking::Client;
use scraper::{html::Html, selector::Selector};
use chrono::{DateTime, TimeZone, NaiveDateTime, Utc};

#[derive(Clone)]
pub struct Question {
    question_id: u64,
    number: String,
    points: f32,
    graders: HashMap<String, usize>,
}

#[derive(Clone)]
pub struct Grader {
    name: String,
    n_questions_graded: usize,
    n_points_graded: f32,
}

#[derive(Clone)]
pub struct Stats {
    course_id: u64,
    assignment_id: u64,
    graders: Vec<Grader>,
    questions: Vec<Question>,
    n_submissions: usize,
    updated_time: DateTime<Utc>
}

impl Stats {
    pub fn new(course_id: u64, assignment_id: u64, http_client: &Client) -> Result<Self, ClientError> {
        /* Create empty stats struct */
        let mut stats = Stats {
            course_id,
            assignment_id,
            graders: vec![],
            questions: vec![],
            n_submissions: 0,
            updated_time: Utc::now(),
        };

        /* Update */
        match stats.refresh(http_client) {
            Ok(()) => Ok(stats),
            Err(e) => Err(e),
        }
    }

    pub fn refresh(&mut self, http_client: &Client) -> Result<(), ClientError> {
        /* Get the number of submissions */
        self.n_submissions = {
            /* Construct the URL */
            let url = format!("https://www.gradescope.com/courses/{}/assignments/{}/submissions",
                              self.course_id, self.assignment_id);

            /* Make the request to the submissions page */
            let response = match http_client.get(&url).send() {
                Ok(response) => Ok(response),
                Err(_) => Err(ClientError::HttpError),
            }?;

            /* Check the status of the response and extract the HTML document */
            let response_body = if response.status() == 200 {
                match response.text() {
                    Ok(text) => Ok(text),
                    Err(_) => Err(ClientError::HttpError)
                }
            } else {
                Err(ClientError::HttpError)
            }?;

            /* Parse the response */
            let document = Html::parse_document(&response_body);

            /* Find all submissions */
            let submissions: Vec<_> = {
                /* Create a selector */
                let selector = Selector::parse(".js-onlineAssignmentSubmissionsTable tbody tr").unwrap();

                /* Find the elements */
                document.select(&selector).collect()
            };

            submissions.len()
        };

        /* Get list of TAs */
        self.graders = {
            /* Construct the URL */
            let url = format!("https://www.gradescope.com/courses/{}/memberships",
                              self.course_id);

            /* Make the request to the roster page */
            let response = match http_client.get(&url).send() {
                Ok(response) => Ok(response),
                Err(_) => Err(ClientError::HttpError),
            }?;

            /* Check the status of the response and extract the HTML document */
            let response_body = if response.status() == 200 {
                match response.text() {
                    Ok(text) => Ok(text),
                    Err(_) => Err(ClientError::HttpError)
                }
            } else {
                Err(ClientError::HttpError)
            }?;

            /* Parse the response */
            let document = Html::parse_document(&response_body);

            /* Find all graders */
            let mut graders: Vec<_> = {
                /* Create a selector */
                let selector = Selector::parse(".js-rosterTable tbody tr").unwrap();
                let role_selector = Selector::parse(".js-rosterRoleSelect option[selected=selected]").unwrap();
                let name_selector = Selector::parse(".js-rosterDeleteUser").unwrap();

                /* Find the elements */
                document.select(&selector).filter(|row| {
                    /* Find selected role */
                    if let Some(role) = row.select(&role_selector).next() {
                        /* Figure out if TA */
                        if let Some("2") = role.value().attr("value") {
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }).map(|row| {
                    if let Some(button) = row.select(&name_selector).next() {
                        if let Some(name) = button.value().attr("data-confirm") {
                            if name.starts_with("Are you sure that you want to remove ") {
                                let name = (&name[37..]).split("?").next().unwrap().to_string();
                                Some(Grader {
                                    name,
                                    n_questions_graded: 0,
                                    n_points_graded: 0.,
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }).filter(Option::is_some).map(Option::unwrap).collect()
            };

            graders.sort_by(|a, b| a.name.cmp(&b.name));
            graders.dedup_by(|a, b| a.name == b.name);

            graders
        };

        /* Get list of assignment questions */
        self.questions = {
            /* Construct the URL */
            let url = format!("https://www.gradescope.com/courses/{}/assignments/{}/grade",
                              self.course_id, self.assignment_id);

            /* Make the request to the assignment grading page */
            let response = match http_client.get(&url).send() {
                Ok(response) => Ok(response),
                Err(_) => Err(ClientError::HttpError),
            }?;

            /* Check the status of the response and extract the HTML document */
            let response_body = if response.status() == 200 {
                match response.text() {
                    Ok(text) => Ok(text),
                    Err(_) => Err(ClientError::HttpError)
                }
            } else {
                Err(ClientError::HttpError)
            }?;

            /* Parse the response */
            let document = Html::parse_document(&response_body);

            /* Get list of questions */
            let questions: Vec<_> = {
                /* Create a selector */
                let selector = Selector::parse(".gradingDashboard .gradingDashboard--question, .gradingDashboard .gradingDashboard--subquestion").unwrap();
                let number_selector = Selector::parse(".gradingDashboard--questionTitle a, .gradingDashboard--subquestionTitle a").unwrap();
                let points_selector = Selector::parse(".gradingDashboard--pointsColumn").unwrap();

                /* Find the elements */
                document.select(&selector).map(|question| {
                    /* Find the id and number */
                    if let Some(title) = question.select(&number_selector).next() {
                        let number = title.inner_html().split(":").next().unwrap().to_string();
                        let question_id = title.value().attr("href").unwrap().split("/questions/").skip(1).next().unwrap().split("/grade").next().unwrap().parse::<u64>().unwrap();

                        /* Get number of points */
                        if let Some(points) = question.select(&points_selector).next() {
                            let points = points.inner_html().parse::<f32>().unwrap();

                            Some(Question {
                                question_id,
                                number,
                                points,
                                graders: HashMap::new(),
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }).filter(Option::is_some).map(Option::unwrap).collect()
            };

            questions
        };

        /* Count number of questions graded per TA */
        for question in self.questions.iter_mut() {
            /* Construct the URL */
            let url = format!("https://www.gradescope.com/courses/{}/questions/{}/submissions",
                              self.course_id, question.question_id);

            /* Make the request to the question grading page */
            let response = match http_client.get(&url).send() {
                Ok(response) => Ok(response),
                Err(_) => Err(ClientError::HttpError),
            }?;

            /* Check the status of the response and extract the HTML document */
            let response_body = if response.status() == 200 {
                match response.text() {
                    Ok(text) => Ok(text),
                    Err(_) => Err(ClientError::HttpError)
                }
            } else {
                Err(ClientError::HttpError)
            }?;

            /* Parse the response */
            let document = Html::parse_document(&response_body);

            /* Get list of submissions */
            {
                /* Create a selector */
                let selector = Selector::parse("#question_submissions tbody tr").unwrap();
                let td_selector = Selector::parse("td").unwrap();

                /* Find the elements */
                for submission in document.select(&selector) {
                    let grader = submission.select(&td_selector).skip(2).next().unwrap().inner_html().to_string();
                    if grader != "" {
                        /* Get grader by name */
                        if let Some(grader) = self.graders.iter_mut().find(|g| g.name == grader) {
                            /* Increment the count for the grader */
                            *question.graders.entry(grader.name.clone()).or_insert(0) += 1;
                            grader.n_questions_graded += 1;
                            grader.n_points_graded += question.points;
                        }
                    }
                }
            }
        }

        self.updated_time = Utc::now();

        Ok(())
    }
}

impl PartialEq for Grader {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Grader {}

impl fmt::Debug for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /* TODO : sort by points graded */
        let mut graders = self.graders.clone();
        graders.sort_by(|a, b| b.n_points_graded.partial_cmp(&a.n_points_graded).unwrap());

        /* Figure out longest name */
        let len = self.graders.iter().fold(0, |acc, g| acc.max(g.name.len()));

        /* Header row */
        write!(f, "{:width$}  ", "", width = len)?;
        let mut textwidth = len + 2;
        for q in self.questions.iter() {
            write!(f, "{:<6}", q.number)?;
            textwidth += 6;
        }
        write!(f, "#?s   pts   \n")?;
        textwidth += 12;

        /* Line */
        write!(f, "{:-<width$}\n", "", width = textwidth)?;

        /* Each grader */
        for g in graders.iter() {
            write!(f, "{:width$}  ", g.name, width = len)?;
            for q in self.questions.iter() {
                write!(f, "{:<6}", q.graders.get(&g.name).unwrap_or(&0))?;
            }
            write!(f, "{:<6}{:<6}\n", g.n_questions_graded, g.n_points_graded)?;
        }
        write!(f, "\nLast Updated: {}\n", self.updated_time);
        
        Ok(())
    }
}
