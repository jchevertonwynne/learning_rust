use async_trait::async_trait;
use grpc::my_service_server::{MyService, MyServiceServer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tonic::transport::Server::builder()
        .add_service(MyServiceServer::new(MyServiceImpl {}))
        .serve(([127, 0, 0, 1], 25565).into())
        .await?;

    Ok(())
}

struct MyServiceImpl {}

#[async_trait]
impl MyService for MyServiceImpl {
    async fn my_method(
        &self,
        request: tonic::Request<grpc::MyRequest>,
    ) -> Result<tonic::Response<grpc::MyResponse>, tonic::Status> {
        let request = request.into_inner();
        let request: Request = match (&request).try_into() {
            Ok(request) => request,
            Err(err) => {
                return Err(tonic::Status::invalid_argument(Into::<&'static str>::into(
                    err,
                )))
            }
        };

        let response = process_request(request);

        Ok(tonic::Response::new(grpc::MyResponse::from(response)))
    }
}

fn process_request(request: Request) -> Response {
    let Request {
        name,
        age,
        earnings,
    } = request;
    let earned = match earnings {
        Earnings::Salaried(s) => s as f32,
        Earnings::Hourly { hours, per_hour } => hours as f32 * per_hour,
    };
    Response {
        acceptance_message: format!("hello {name}, age {age}. you earned £{earned}"),
    }
}

struct Response {
    acceptance_message: String,
}

impl From<Response> for grpc::MyResponse {
    fn from(value: Response) -> Self {
        let Response { acceptance_message } = value;
        grpc::MyResponse { acceptance_message }
    }
}

struct Request<'a> {
    name: &'a str,
    age: i32,
    earnings: Earnings,
}

enum Earnings {
    Salaried(i32),
    Hourly { hours: i32, per_hour: f32 },
}

#[derive(Debug, Clone, Copy)]
enum MyRequestError {
    InvalidName,
    InvalidAge,
    NonPresentEarnings,
    InvalidSalary,
    InvalidHours,
    InvalidPerHours,
}

impl From<MyRequestError> for &'static str {
    fn from(value: MyRequestError) -> Self {
        match value {
            MyRequestError::InvalidName => "name must be a non-empty string",
            MyRequestError::InvalidAge => "age must be 0 or greater",
            MyRequestError::NonPresentEarnings => "earnings field must be supplied",
            MyRequestError::InvalidSalary => "salary must be greater than 0",
            MyRequestError::InvalidHours => "hours must be 0 or greater",
            MyRequestError::InvalidPerHours => "per hour must be greater than 0",
        }
    }
}

impl<'a> TryFrom<&'a grpc::MyRequest> for Request<'a> {
    type Error = MyRequestError;

    fn try_from(value: &'a grpc::MyRequest) -> Result<Self, Self::Error> {
        let grpc::MyRequest {
            name,
            age,
            earnings,
        } = value;
        let name = name.trim();
        if name.is_empty() {
            return Err(MyRequestError::InvalidName);
        }

        let age = *age;
        if age < 0 {
            return Err(MyRequestError::InvalidAge);
        }

        let Some(earnings) = earnings else {
            return Err(MyRequestError::NonPresentEarnings);
        };
        let earnings = match earnings {
            grpc::my_request::Earnings::Salaried(salary) => {
                if *salary <= 0 {
                    return Err(MyRequestError::InvalidSalary);
                }
                Earnings::Salaried(*salary)
            }
            grpc::my_request::Earnings::Hourly(grpc::Hourly { hours, per_hour }) => {
                let hours = *hours;
                let per_hour = *per_hour;
                if hours < 0 {
                    return Err(MyRequestError::InvalidHours);
                }
                if per_hour <= 0. {
                    return Err(MyRequestError::InvalidPerHours);
                }
                Earnings::Hourly { hours, per_hour }
            }
        };

        Ok(Request {
            name,
            age,
            earnings,
        })
    }
}
