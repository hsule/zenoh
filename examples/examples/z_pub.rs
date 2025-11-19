//
// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//
use clap::Parser;
use std::{time::{Duration, SystemTime, UNIX_EPOCH}};
// use prost::bytes::buf;
use zenoh::{bytes::Encoding, key_expr::KeyExpr, Config};
use zenoh_examples::CommonArgs;

#[tokio::main]
async fn main() {
    // Initiate logging
    zenoh::init_log_from_env_or("error");

    let (config, key_expr, payload, attachment, add_matching_listener) = parse_args();

    println!("Opening session...");
    let session = zenoh::open(config).await.unwrap();

    println!("Declaring Publisher on '{key_expr}'...");
    let publisher = session.declare_publisher(&key_expr).await.unwrap();

    if add_matching_listener {
        publisher
            .matching_listener()
            .callback(|matching_status| {
                if matching_status.matching() {
                    println!("Publisher has matching subscribers.");
                } else {
                    println!("Publisher has NO MORE matching subscribers.");
                }
            })
            .background()
            .await
            .unwrap();
    }

    const TARGET_MBPS: u128 = 2; // 目標 2 Mbps
    const PERIOD_MS: u64 = 50;

    let bits_per_sec = TARGET_MBPS * 1_000_000u128;
    let bits_per_interval = bits_per_sec * (PERIOD_MS as u128) / 1000u128;
    let bytes_per_interval = (bits_per_interval / 8u128) as usize;

    println!("Waiting for 5 seconds before starting publish loop...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    println!("Press CTRL-C to quit...");
    for idx in 0..u32::MAX {
        tokio::time::sleep(Duration::from_millis(PERIOD_MS)).await;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let now_ns = (now.as_secs() as u128) * 1_000_000_000 + now.subsec_nanos() as u128;

        let prefix = format!("[{:4}] ts_ns={} ", idx, now_ns);
        let payload_len = bytes_per_interval.saturating_sub(prefix.len());

        let payload: String = std::iter::repeat('A').take(payload_len).collect();
        let buf = prefix + &payload;


        println!("Putting Data ('{}': '{}')...", &key_expr, buf);
        // Refer to z_bytes.rs to see how to serialize different types of message
        publisher
            .put(buf)
            .encoding(Encoding::TEXT_PLAIN) // Optionally set the encoding metadata 
            .attachment(attachment.clone()) // Optionally add an attachment
            .await
            .unwrap();
    }
}

#[derive(clap::Parser, Clone, PartialEq, Eq, Hash, Debug)]
struct Args {
    #[arg(short, long, default_value = "demo/example/zenoh-rs-pub")]
    /// The key expression to write to.
    key: KeyExpr<'static>,
    #[arg(short, long, default_value = "Pub from Rust!")]
    /// The payload to write.
    payload: String,
    #[arg(short, long)]
    /// The attachments to add to each put.
    attach: Option<String>,
    /// Enable matching listener.
    #[arg(long)]
    add_matching_listener: bool,
    #[command(flatten)]
    common: CommonArgs,
}

fn parse_args() -> (Config, KeyExpr<'static>, String, Option<String>, bool) {
    let args = Args::parse();
    (
        args.common.into(),
        args.key,
        args.payload,
        args.attach,
        args.add_matching_listener,
    )
}
