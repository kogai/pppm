extern crate bodyparser;
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate failure;
extern crate iron;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
use exonum::api::{Api, ApiError};
use exonum::blockchain::{ApiContext, Blockchain, ExecutionError, ExecutionResult, GenesisConfig,
                         Service, Transaction, TransactionSet, ValidatorKeys};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::encoding::serialize::FromHex;
use exonum::messages::{Message, RawTransaction};
use exonum::node::{ApiSender, Node, NodeApiConfig, NodeConfig, TransactionSend};
use exonum::storage::{Fork, MapIndex, MemoryDB, Snapshot};
use iron::Handler;
use iron::headers::ContentType;
use iron::modifiers::Header;
use iron::prelude::*;
use iron::status::Status;
use router::Router;

// Service identifier
const SERVICE_ID: u16 = 1;
// Starting balance of a newly created wallet
const INIT_BALANCE: u64 = 100;

encoding_struct! {
  struct Wallet {
      pub_key: &PublicKey,
      name: &str,
      balance: u64,
  }
}

impl Wallet {
  pub fn increase(self, amount: u64) -> Self {
    let balance = self.balance() + amount;
    Self::new(self.pub_key(), self.name(), balance)
  }

  pub fn decrease(self, amount: u64) -> Self {
    let balance = self.balance() - amount;
    Self::new(self.pub_key(), self.name(), balance)
  }
}

pub struct CurrencySchema<T> {
  view: T,
}

impl<T: AsRef<Snapshot>> CurrencySchema<T> {
  pub fn new(view: T) -> Self {
    CurrencySchema { view }
  }

  pub fn wallets(&self) -> MapIndex<&Snapshot, PublicKey, Wallet> {
    MapIndex::new("cryptocurrency.wallets", self.view.as_ref())
  }

  // Utility method to quickly get a separate wallet from the storage
  pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
    self.wallets().get(pub_key)
  }
}

impl<'a> CurrencySchema<&'a mut Fork> {
  pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
    MapIndex::new("cryptocurrency.wallets", &mut self.view)
  }
}

transactions! {
  // Transaction group.
  pub CurrencyTransactions {
    const SERVICE_ID = SERVICE_ID;

    // Transaction type for creating a new wallet.
    struct TxCreateWallet {
      pub_key: &PublicKey,
      name: &str,
    }

    // Transaction type for transferring tokens between two wallets.
    struct TxTransfer {
      from: &PublicKey,
      to: &PublicKey,
      amount: u64,
      seed: u64,
    }
  }
}

#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
  #[fail(display = "Wallet already exists")]
  WalletAlreadyExists = 0,

  #[fail(display = "Sender doesn't exist")]
  SenderNotFound = 1,

  #[fail(display = "Receiver doesn't exist")]
  ReceiverNotFound = 2,

  #[fail(display = "Insufficient currency amount")]
  InsufficientCurrencyAmount = 3,
}

// Conversion between service-specific errors and the standard error type
// that can be emitted by transactions.
impl From<Error> for ExecutionError {
  fn from(value: Error) -> ExecutionError {
    let description = format!("{}", value);
    ExecutionError::with_description(value as u8, description)
  }
}

impl Transaction for TxCreateWallet {
  fn verify(&self) -> bool {
    self.verify_signature(self.pub_key())
  }

  fn execute(&self, view: &mut Fork) -> ExecutionResult {
    let mut schema = CurrencySchema::new(view);
    if schema.wallet(self.pub_key()).is_none() {
      let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE);
      println!("Create the wallet: {:?}", wallet);
      schema.wallets_mut().put(self.pub_key(), wallet);
      Ok(())
    } else {
      Err(Error::WalletAlreadyExists)?
    }
  }
}

impl Transaction for TxTransfer {
  fn verify(&self) -> bool {
    (*self.from() != *self.to()) && self.verify_signature(self.from())
  }

  fn execute(&self, view: &mut Fork) -> ExecutionResult {
    let mut schema = CurrencySchema::new(view);

    let sender = match schema.wallet(self.from()) {
      Some(val) => val,
      None => Err(Error::SenderNotFound)?,
    };

    let receiver = match schema.wallet(self.to()) {
      Some(val) => val,
      None => Err(Error::ReceiverNotFound)?,
    };

    let amount = self.amount();
    if sender.balance() >= amount {
      let sender = sender.decrease(amount);
      let receiver = receiver.increase(amount);
      println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
      let mut wallets = schema.wallets_mut();
      wallets.put(self.from(), sender);
      wallets.put(self.to(), receiver);
      Ok(())
    } else {
      Err(Error::InsufficientCurrencyAmount)?
    }
  }
}

#[derive(Clone)]
struct CryptocurrencyApi {
  channel: ApiSender,
  blockchain: Blockchain,
}

#[derive(Serialize, Deserialize)]
pub struct TransactionResponse {
  // Hash of the transaction.
  pub tx_hash: Hash,
}

impl CryptocurrencyApi {
  fn post_transaction(&self, req: &mut Request) -> IronResult<Response> {
    match req.get::<bodyparser::Struct<CurrencyTransactions>>() {
      Ok(Some(transaction)) => {
        let transaction: Box<Transaction> = transaction.into();
        let tx_hash = transaction.hash();
        self.channel.send(transaction).map_err(ApiError::from)?;
        let json = TransactionResponse { tx_hash };
        self.ok_response(&serde_json::to_value(&json).unwrap())
      }
      Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
      Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
    }
  }
}

impl CryptocurrencyApi {
  fn get_wallet(&self, req: &mut Request) -> IronResult<Response> {
    let path = req.url.path();
    let wallet_key = path.last().unwrap();
    let public_key = PublicKey::from_hex(wallet_key).map_err(|e| {
      IronError::new(
        e,
        (
          Status::BadRequest,
          Header(ContentType::json()),
          "\"Invalid request param: `pub_key`\"",
        ),
      )
    })?;

    let snapshot = self.blockchain.snapshot();
    let schema = CurrencySchema::new(snapshot);

    if let Some(wallet) = schema.wallet(&public_key) {
      self.ok_response(&serde_json::to_value(wallet).unwrap())
    } else {
      self.not_found_response(&serde_json::to_value("Wallet not found").unwrap())
    }
  }

  fn get_wallets(&self, _: &mut Request) -> IronResult<Response> {
    let snapshot = self.blockchain.snapshot();
    let schema = CurrencySchema::new(snapshot);
    let idx = schema.wallets();
    let wallets: Vec<Wallet> = idx.values().collect();

    self.ok_response(&serde_json::to_value(&wallets).unwrap())
  }
}

impl Api for CryptocurrencyApi {
  fn wire(&self, router: &mut Router) {
    let self_ = self.clone();
    let post_create_wallet = move |req: &mut Request| self_.post_transaction(req);
    let self_ = self.clone();
    let post_transfer = move |req: &mut Request| self_.post_transaction(req);
    let self_ = self.clone();
    let get_wallets = move |req: &mut Request| self_.get_wallets(req);
    let self_ = self.clone();
    let get_wallet = move |req: &mut Request| self_.get_wallet(req);

    // Bind handlers to specific routes.
    router.post("/v1/wallets", post_create_wallet, "post_create_wallet");
    router.post("/v1/wallets/transfer", post_transfer, "post_transfer");
    router.get("/v1/wallets", get_wallets, "get_wallets");
    router.get("/v1/wallet/:pub_key", get_wallet, "get_wallet");
  }
}

pub struct CurrencyService;

impl Service for CurrencyService {
  fn service_name(&self) -> &'static str {
    "cryptocurrency"
  }

  fn service_id(&self) -> u16 {
    SERVICE_ID
  }

  fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
    let tx = CurrencyTransactions::tx_from_raw(raw)?;
    Ok(tx.into())
  }

  fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
    vec![]
  }

  fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
    let mut router = Router::new();
    let api = CryptocurrencyApi {
      channel: ctx.node_channel().clone(),
      blockchain: ctx.blockchain().clone(),
    };
    api.wire(&mut router);
    Some(Box::new(router))
  }
}

fn node_config() -> NodeConfig {
  // Code goes here
  let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
  let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();

  let validator_keys = ValidatorKeys {
    consensus_key: consensus_public_key,
    service_key: service_public_key,
  };
  let genesis = GenesisConfig::new(vec![validator_keys].into_iter());
  let api_address = "0.0.0.0:8000".parse().unwrap();
  let api_cfg = NodeApiConfig {
    public_api_address: Some(api_address),
    ..Default::default()
  };
  let peer_address = "0.0.0.0:2000".parse().unwrap();

  // Return this value from `node_config` function
  NodeConfig {
    listen_address: peer_address,
    peers: vec![],
    service_public_key,
    service_secret_key,
    consensus_public_key,
    consensus_secret_key,
    genesis,
    external_address: None,
    network: Default::default(),
    whitelist: Default::default(),
    api: api_cfg,
    mempool: Default::default(),
    services_configs: Default::default(),
  }
}

/**
 * TODO:
 * add
 * remove
 * list
 * find
 * vote
 * init
 */
fn main() {
  exonum::helpers::init_logger().unwrap();

  println!("Creating in-memory database...");
  let node = Node::new(
    MemoryDB::new(),
    vec![Box::new(CurrencyService)],
    node_config(),
  );
  println!("Starting a single node...");
  println!("Blockchain is ready for transactions!");
  node.run().unwrap();
}

// fn main() {
//   exonum::helpers::init_logger().unwrap();
//   let node = Node::new(
//     MemoryDB::new(),
//     vec![Box::new(CurrencyService)],
//     node_config(),
//   );
//   node.run().unwrap();
// }
