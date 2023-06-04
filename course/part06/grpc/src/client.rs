use grpc::{my_service_client::MyServiceClient, Hourly, MyRequest, my_request::Earnings};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = MyServiceClient::connect("http://127.0.0.1:25565").await?;

    let response = client
        .my_method(MyRequest {
            name: "joseph".into(),
            age: 25,
            earnings: Some(Earnings::Hourly(Hourly {
                hours: 5,
                per_hour: 10.,
            })),
        })
        .await?
        .into_inner();
    println!("response = {response:?}");

    let response = client
        .my_method(MyRequest {
            name: "joseph2".into(),
            age: 26,
            earnings: Some(Earnings::Salaried(123)),
        })
        .await?
        .into_inner();
    println!("response = {response:?}");

    Ok(())
}
