use api::v1;
use kube::CustomResourceExt;

mod api;

fn main() {
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&v1::dds_recv::DdsConnection::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&v1::drgs::DrgsConnection::crd()).unwrap()
    );
}
