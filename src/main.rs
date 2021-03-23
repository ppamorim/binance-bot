use std::sync::atomic::{AtomicBool,  Ordering};
use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use binance::api::Binance;
use binance::account::Account;
use binance::model::Order;
use binance::websockets::*;

use atomic_float::AtomicF64;
use core::sync::atomic::Ordering::Relaxed;

fn main() {

    let account: Account = load_account();

    let symbol = "DENTUSDT";
    let margin: f32 = 0.01;

    let symbol_c = symbol.clone();
    let margin_c = margin.clone();
    let acc_c = account.clone();

    let price_monitor_thread = std::thread::spawn(move || {
        price_monitor(account, symbol, margin);
    });

    let user_monitor_thread = std::thread::spawn(move || {
        user_monitor(acc_c, symbol_c, margin_c);
    });

    let _ = price_monitor_thread.join();
    let _ = user_monitor_thread.join();

}

fn price_monitor(account: Account, symbol: &str, margin: f32) {

    let order_found = AtomicBool::new(false);

    static HIGHEST_PRICE: AtomicF64 = AtomicF64::new(0.0);

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

                        println!("Latest close price: {}", symbol_close);
                        
                        match get_open_order(&account, symbol) {
                            Some(order) => {

                                order_found.store(true, Ordering::Relaxed);
        
                                let margin_price: f64 = f64::from(symbol_close * margin);
                                let recommended_price_stop: f64 = f64::from(symbol_close) - margin_price;

                                let highest_recommended_price_stop: f64 = HIGHEST_PRICE.load(Ordering::Relaxed);
                                let max_recommended_price_stop: f64 = highest_recommended_price_stop.max(recommended_price_stop);
                                HIGHEST_PRICE.store(max_recommended_price_stop, Ordering::Relaxed);
        
                                println!("Close: {}, Recommended price: {}", symbol_close, max_recommended_price_stop);

                                let diff: f64 = f64::from(symbol_close) - order.price;
                                println!("diff {}, {}", diff, margin_price);
                                if diff > margin_price {
                                    update_stop_loss(&account, order, max_recommended_price_stop);
                                } else {
                                    println!("Keep stop loss");
                                }
                            },
                            None => {
                                let require_notify = order_found.load(Ordering::Relaxed);
                                order_found.store(false, Ordering::Relaxed);
                                if require_notify {
                                    println!("____________________________________________________________\n");
                                    println!("Order not found");
                                    println!("____________________________________________________________");
                                }
                            },
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

use binance::userstream::*;

fn user_monitor(account: Account, symbol: &str, margin: f32) {

    let user_stream: UserStream = user_stream();
    let keep_running = AtomicBool::new(true);

    if let Ok(answer) = user_stream.start() {

        let listen_key = answer.listen_key;
    
        let mut web_socket: WebSockets = WebSockets::new(|event: WebsocketEvent| {
            match event {
            WebsocketEvent::AccountUpdate(account_update) => {
                for balance in &account_update.balance {
                println!("Asset: {}, free: {}, locked: {}", balance.asset, balance.free, balance.locked);
                }
            },
            WebsocketEvent::OrderTrade(trade) => {
                println!("Symbol: {}, Side: {}, Price: {}, Execution Type: {}", trade.symbol, trade.side, trade.price, trade.execution_type);
            },
            _ => (),
            };
            Ok(())
        });
    
        web_socket.connect(&listen_key).unwrap(); // check error
            if let Err(e) = web_socket.event_loop(&keep_running) {
            match e {
                err => {
                    println!("Error: {:?}", err);
                }
            }
        }

        return
    }

    println!("Not able to start an User Stream (Check your API_KEY)");
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    api_key: String,
    secret_key: String,
}

fn load_account() -> Account {
    let mut file = File::open("env.toml").unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();
    let config: Config = toml::from_str(&buffer).unwrap();
    let api_key = Some(config.api_key.into());
    let secret_key = Some(config.secret_key.into());
    Binance::new(api_key, secret_key)
}

fn user_stream() -> UserStream {
    let mut file = File::open("env.toml").unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();
    let config: Config = toml::from_str(&buffer).unwrap();
    let api_key = Some(config.api_key.into());
    let secret_key = Some(config.secret_key.into());
    Binance::new(api_key, secret_key)
}

fn get_open_order(account: &Account, symbol: &str) -> Option<Order> {
    match account.get_open_orders(symbol) {
        Ok(open_orders) => {
            if open_orders.is_empty() {
                return None
            }
            Some(open_orders[0].clone())
        },
        Err(e) => None,
    }
}

fn update_stop_loss(account: &Account, order: Order, recommended_price_stop: f64) {
    println!("____________________________________________________________\n");
    println!("Updating stop loss! Recommended price: {}", recommended_price_stop);
    println!("____________________________________________________________");
    let latest_order: Order = order.clone();
    match cancel_current_stop_loss_order(account, order) {
        Ok(_) => {
            create_stop_loss_order(account, latest_order, recommended_price_stop); 
        },
        Err(e) => println!("Error: {:?}", e),
    }
}

fn cancel_current_stop_loss_order(account: &Account, order: Order) -> Result<(), binance::errors::Error> {
    match account.cancel_order(order.symbol, order.order_id) {
        Ok(answer) => {
            println!("____________________________________________________________\n");
            println!("Current order at price {} cancelled", order.price);
            println!("____________________________________________________________");
            Ok(())
        },
        Err(e) => Err(e),
    }
}

fn create_stop_loss_order(account: &Account, latest_order: Order, recommended_price_stop: f64) {

    let latest_margin: f64 = latest_order.stop_price - latest_order.price;
    let recommended_price_limit: f64 = recommended_price_stop - latest_margin;

    let orig_qty: f64 = latest_order.orig_qty.parse::<f64>().unwrap();

    println!("____________________________________________________________\n");
    println!("Updating Symbol {}\nAmount: {}\nRecommended price stop: {}\nRecommended price limit: {}", latest_order.symbol, orig_qty, recommended_price_stop, recommended_price_limit);
    println!("____________________________________________________________");

    let result = account.stop_limit_sell_order(
        latest_order.symbol, 
        orig_qty, 
        recommended_price_limit,
        recommended_price_stop, 
        binance::account::TimeInForce::GTC);

    match result {
        Ok(answer) => println!("{:?}", answer),
        Err(e) => println!("Error: {:?}", e),
    }
}
