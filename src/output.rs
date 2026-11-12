// Wire definitions mirror proto/transfers.proto.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Transfers {
    #[prost(message, repeated, tag = "1")]
    pub transfers: Vec<Transfer>,
    #[prost(uint64, tag = "2")]
    pub slot: u64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Transfer {
    #[prost(string, tag = "1")]
    pub from: String,
    #[prost(string, tag = "2")]
    pub to: String,
    #[prost(uint64, tag = "3")]
    pub amount: u64,
}

