use std::sync::atomic::{AtomicBool};
use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use binance::api::Binance;
use binance::account::Account;
use binance::model::Order;
use binance::websockets::*;

fn main() {

    let account: Account = load_account();

    let symbol = "DENTUSDT";
    let margin: f32 = 0.1;

    let keep_running = AtomicBool::new(true); // Used to control the event loop
    let agg_trade: String = format!("!ticker@arr"); // All Symbols
    let mut web_socket: WebSockets = WebSockets::new(|event: WebsocketEvent| {
        match event {
            // 24hr rolling window ticker statistics for all symbols that changed in an array.
            WebsocketEvent::DayTickerAll(ticker_events) => {
                for tick_event in ticker_events {
                    if tick_event.symbol == symbol {

                        // let symbol_average: f32 = tick_event.average_price.parse().unwrap();
                        let symbol_close: f32 = tick_event.current_close.parse().unwrap();

                        let margin_price: f64 = f64::from(symbol_close * margin);
                        let recommended_price_stop: f64 = f64::from(symbol_close) - margin_price;
                        
                        match get_open_order(&account, symbol) {
                            Some(order) => {
                                let diff: f64 = f64::from(symbol_close) - order.price;
                                println!("Close: {}, Order price: {}, Diff: {}", symbol_close, order.price, diff);

                                if diff > margin_price {
                                    update_stop_loss(&account, order, recommended_price_stop);
                                } else {
                                    println!("Keep stop loss");
                                }

                            },
                            None => println!("Order not found"),
                        };

                        

                    }
                }
            },
            _ => (),
        };
        Ok(())
    });

    web_socket.connect(&agg_trade).unwrap(); // check error
    if let Err(e) = web_socket.event_loop(&keep_running) {
        match e {
            err => {
                println!("Error: {:?}", err);
            }
        }
    }

}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    api_key: String,
    secret_key: String,
}

fn load_account() -> Account {
    let mut file = File::open("env.toml").unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();
    let config: Config = toml::from_str(&s).unwrap();

    let api_key = Some(config.api_key.into());
    let secret_key = Some(config.secret_key.into());
    Binance::new(api_key, secret_key)
}

fn get_open_order(account: &Account, symbol: &str) -> Option<Order> {
    match account.get_open_orders(symbol) {
        Ok(open_orders) => Some(open_orders[0].clone()),
        Err(e) => None,
    }
}

fn update_stop_loss(account: &Account, order: Order, recommended_price_stop: f64) {
    println!("Updating stop loss! Recommended price: {}", recommended_price_stop);
}