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

use console::Term;
use dialoguer::{theme::ColorfulTheme, Input, Confirm};
use chrono::prelude::*;

fn main() {

    let term = Term::stdout();

    let ascii_name = r#"
888888b.   d8b                                                  888888b.            888    
888  "88b  Y8P                                                  888  "88b           888    
888  .88P                                                       888  .88P           888    
8888888K.  888 88888b.   8888b.  88888b.   .d8888b .d88b.       8888888K.   .d88b.  888888 
888  "Y88b 888 888 "88b     "88b 888 "88b d88P"   d8P  Y8b      888  "Y88b d88""88b 888    
888    888 888 888  888 .d888888 888  888 888     88888888      888    888 888  888 888    
888   d88P 888 888  888 888  888 888  888 Y88b.   Y8b.          888   d88P Y88..88P Y88b.  
8888888P"  888 888  888 "Y888888 888  888  "Y8888P "Y8888       8888888P"   "Y88P"   "Y888
"#;

    term.write_line(ascii_name).unwrap();

    let local: DateTime<Local> = Local::now();
    let greetings_hour: String = match local.hour() {
        0..=11 => String::from("morning"),
        12..=17 => String::from("afternoon"),
        _ => String::from("night"),
    };

    let greetings = format!("Good {}. Welcome to the Binance bot!\n", greetings_hour);
    term.write_line(&greetings).unwrap();

    let symbol: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("What is the symbol (i.e. BTCUSDT)?")
        .interact_text()
        .unwrap();

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Do you confirm the symbol {}?", symbol))
        .interact()
        .unwrap() {
        panic!("Operation cancelled. Reason: Symbol not confirmed.");
    }

    let margin_string: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("What is the margin for the symbol {}?", symbol))
        .interact_text()
        .unwrap();

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Do you confirm the margin of {}% to the symbol {}?", margin_string, symbol))
        .interact()
        .unwrap() {
        panic!("Operation cancelled. Reason: Margin not confirmed.");
    }

    let margin: f32 = (margin_string.parse::<f32>().unwrap())/100.0;

    let account: Account = load_account();

    let symbol_c = symbol.clone();
    let margin_c = margin.clone();
    let acc_c = account.clone();

    let price_monitor_thread = std::thread::spawn(move || {
        price_monitor(account, &symbol, margin);
    });

    let user_monitor_thread = std::thread::spawn(move || {
        user_monitor(acc_c, &symbol_c, margin_c);
    });

    let _ = price_monitor_thread.join();
    let _ = user_monitor_thread.join();

}

fn price_monitor(account: Account, symbol: &str, margin: f32) {

    let order_found = AtomicBool::new(false);

    static HIGHEST_PRICE: AtomicF64 = AtomicF64::new(0.0);
    
    // let agg_trade: String = format!("!ticker@arr"); // All Symbols
    let agg_trade: String = format!("{}@ticker", symbol.to_lowercase()); // All Symbols

    let keep_running = AtomicBool::new(true); // Used to control the event loop
    let mut web_socket: WebSockets = WebSockets::new(|event: WebsocketEvent| {
        match event {

            WebsocketEvent::DayTicker(ticker_event) => {
                // let symbol_average: f32 = tick_event.average_price.parse().unwrap();
                let symbol_close: f32 = ticker_event.current_close.parse().unwrap();

                println!("Latest close price: {}", symbol_close);
                
                match get_open_order(&account, symbol) {
                    Some(order) => {

                        order_found.store(true, Ordering::Relaxed);

                        let margin_price: f64 = f64::from(symbol_close * margin);
                        let recommended_price_stop: f64 = f64::from(symbol_close) - margin_price;

                        let highest_recommended_price_stop: f64 = HIGHEST_PRICE.load(Ordering::Relaxed);
                        let max_recommended_price_stop: f64 = highest_recommended_price_stop.max(recommended_price_stop);
                        HIGHEST_PRICE.store(max_recommended_price_stop, Ordering::Relaxed);

                        let diff: f64 = f64::from(symbol_close) - order.price;

                        if diff > margin_price {
                            update_stop_loss(&account, order, max_recommended_price_stop);
                        }
                    },
                    None => {
                        // // let require_notify = order_found.load(Ordering::Relaxed);
                        // // order_found.store(false, Ordering::Relaxed);
                        // // if require_notify {
                        //     println!("____________________________________________________________\n");
                        //     println!("Order not found");
                        //     println!("____________________________________________________________");
                        // // }
                    },
                };
            },

            // 24hr rolling window ticker statistics for all symbols that changed in an array.
            // WebsocketEvent::DayTickerAll(ticker_events) => {
            //     for tick_event in ticker_events {
            //         if tick_event.symbol == symbol {

            //             // let symbol_average: f32 = tick_event.average_price.parse().unwrap();
            //             let symbol_close: f32 = tick_event.current_close.parse().unwrap();

            //             println!("Latest close price: {}", symbol_close);
                        
            //             match get_open_order(&account, symbol) {
            //                 Some(order) => {

            //                     order_found.store(true, Ordering::Relaxed);
        
            //                     let margin_price: f64 = f64::from(symbol_close * margin);
            //                     let recommended_price_stop: f64 = f64::from(symbol_close) - margin_price;

            //                     let highest_recommended_price_stop: f64 = HIGHEST_PRICE.load(Ordering::Relaxed);
            //                     let max_recommended_price_stop: f64 = highest_recommended_price_stop.max(recommended_price_stop);
            //                     HIGHEST_PRICE.store(max_recommended_price_stop, Ordering::Relaxed);

            //                     let diff: f64 = f64::from(symbol_close) - order.price;

            //                     if diff > margin_price {
            //                         update_stop_loss(&account, order, max_recommended_price_stop);
            //                     }
            //                 },
            //                 None => {
            //                     // // let require_notify = order_found.load(Ordering::Relaxed);
            //                     // // order_found.store(false, Ordering::Relaxed);
            //                     // // if require_notify {
            //                     //     println!("____________________________________________________________\n");
            //                     //     println!("Order not found");
            //                     //     println!("____________________________________________________________");
            //                     // // }
            //                 },
            //             };

                        

            //         }
            //     }
            // },
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
                // match trade.execution_type {

                // }
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

    let t_recommended_price_limit = truncate(recommended_price_limit);
    let t_recommended_price_stop = truncate(recommended_price_stop);

    println!("____________________________________________________________\n");
    println!("Updating Symbol {}\nAmount: {}\nRecommended price stop: {}\nRecommended price limit: {}",
     latest_order.symbol, orig_qty, t_recommended_price_stop, t_recommended_price_limit);
    println!("____________________________________________________________");

    let result = account.stop_limit_sell_order(
        latest_order.symbol, 
        orig_qty, 
        t_recommended_price_limit,
        t_recommended_price_stop, 
        binance::account::TimeInForce::GTC);

    match result {
        Ok(answer) => println!("{:?}", answer),
        Err(e) => println!("Error: {:?}", e),
    }
}

fn truncate(value: f64) -> f64 {
    f64::trunc(value*10000000.0)/10000000.0
}