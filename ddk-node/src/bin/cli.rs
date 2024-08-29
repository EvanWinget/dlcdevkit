use clap::{Parser, Subcommand};
use ddk::bdk::bitcoin::{Address, Transaction};
use ddk::bdk::LocalOutput;
use ddk::dlc_manager::contract::contract_input::ContractInput;
use ddk::dlc_manager::contract::offered_contract::OfferedContract;
use ddk_node::ddkrpc::ddk_rpc_client::DdkRpcClient;
use ddk_node::ddkrpc::{
    AcceptOfferRequest, GetWalletTransactionsRequest, InfoRequest, ListOffersRequest,
    ListUtxosRequest, NewAddressRequest, SendOfferRequest, WalletBalanceRequest,
};
use inquire::Text;

#[derive(Debug, Clone, Parser)]
#[clap(name = "ddk")]
#[command(about = "A CLI tool for DDK", version = "1.0")]
struct DdkCliArgs {
    #[clap(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum CliCommand {
    // Gets information about the DDK instance
    Info,
    // Pass a contract input to send an offer
    OfferContract(Offer),
    // Retrieve the offers that ddk-node has received.
    Offers,
    // Accept a DLC offer with the contract id string.
    AcceptOffer(Accept),
    // Wallet commands
    #[clap(subcommand)]
    Wallet(WalletCommand),
}

#[derive(Parser, Clone, Debug)]
struct Offer {
    #[arg(help = "Path to a contract input file. Eventually to be a repl asking contract params")]
    #[arg(short = 'f', long = "file")]
    pub contract_input_file: Option<String>,
}

#[derive(Clone, Debug, Subcommand)]
enum WalletCommand {
    #[command(about = "Get the wallet balance.")]
    Balance,
    #[command(about = "Generate a new, unused address from the wallet.")]
    NewAddress,
    #[command(about = "Get the wallet transactions.")]
    Transactions,
    #[command(about = "Get the wallet utxos.")]
    Utxos,
}

#[derive(Parser, Clone, Debug)]
struct Accept {
    // The contract id string to accept.
    pub contract_id: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = DdkCliArgs::parse();

    let mut client = DdkRpcClient::connect("http://127.0.0.1:3030").await?;

    match args.command {
        CliCommand::Info => {
            let info = client.info(InfoRequest::default()).await?.into_inner();
            println!("{:?}", info);
        }
        CliCommand::OfferContract(arg) => {
            let contract_input = if let Some(file) = arg.contract_input_file {
                let contract_string = std::fs::read_to_string(file)?;
                serde_json::from_str::<ContractInput>(&contract_string)?
            } else {
                let offer_collateral: u64 = Text::new("Collateral from you? (sats)").prompt()?.parse()?;
                let accept_collateral: u64 = Text::new("Collateral from counterparty? (sats)").prompt()?.parse()?;
                let fee_rate: u64 = Text::new("Fee rate? (sats/vbyte)").prompt()?.parse()?;
                let min_price: u64 = Text::new("Minimum Bitcoin price?").prompt()?.parse()?;
                let max_price: u64 = Text::new("Maximum Bitcoin price?").prompt()?.parse()?;
                let num_steps: u64 = Text::new("Number of rounding steps?").prompt()?.parse()?;
                ddk_payouts::create_contract_input(min_price, max_price, num_steps, offer_collateral, accept_collateral, fee_rate)
            };

            println!("{:?}", contract_input)
        }
        CliCommand::Offers => {
            let offers_request = client.list_offers(ListOffersRequest {}).await?.into_inner();
            let offers: Vec<OfferedContract> = offers_request
                .offers
                .iter()
                .map(|offer| serde_json::from_slice(offer).unwrap())
                .collect();
            for offer in offers {
                println!("Contract: {}", hex::encode(&offer.id));
            }
        }
        CliCommand::AcceptOffer(accept) => {
            let accept = client
                .accept_offer(AcceptOfferRequest {
                    contract_id: accept.contract_id,
                })
                .await?
                .into_inner();
            println!("Contract Accepted w/ node id: {:?}", accept.node_id)
        }
        CliCommand::Wallet(wallet) => match wallet {
            WalletCommand::Balance => {
                let balance = client
                    .wallet_balance(WalletBalanceRequest::default())
                    .await?
                    .into_inner();
                println!("Balance: {:?}", balance);
            }
            WalletCommand::NewAddress => {
                let address = client
                    .new_address(NewAddressRequest::default())
                    .await?
                    .into_inner();
                println!("{:?}", address)
            }
            WalletCommand::Transactions => {
                let transactions = client
                    .get_wallet_transactions(GetWalletTransactionsRequest::default())
                    .await?
                    .into_inner();
                for tx in transactions.transactions {
                    let transaction: Transaction = serde_json::from_slice(&tx.transaction)?;
                    println!("TxId: {:?}", transaction.txid().to_string());
                    for output in transaction.output {
                        println!(
                            "\t\tValue: {:?}\tAddress: {:?}",
                            output.value,
                            Address::from_script(
                                &output.script_pubkey,
                                ddk::bdk::bitcoin::Network::Regtest
                            )
                        )
                    }
                }
            }
            WalletCommand::Utxos => {
                let utxos = client
                    .list_utxos(ListUtxosRequest::default())
                    .await?
                    .into_inner();
                for utxo in utxos.utxos {
                    let utxo: LocalOutput = serde_json::from_slice(&utxo)?;
                    println!(
                        "TxId: {:?} Index: {:?}",
                        utxo.outpoint.txid, utxo.outpoint.vout
                    );
                    println!(
                        "\t\tAddress: {:?}",
                        Address::from_script(
                            &utxo.txout.script_pubkey,
                            ddk::bdk::bitcoin::Network::Regtest
                        )
                    );
                    println!("\t\tValue: {:?}", utxo.txout.value);
                }
            }
        },
    }

    Ok(())
}
