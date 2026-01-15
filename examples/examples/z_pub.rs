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
use zenoh::{bytes::Encoding, key_expr::KeyExpr, qos::{CongestionControl, Priority}, Config};
use zenoh_examples::CommonArgs;

#[tokio::main]
async fn main() {
    // Initiate logging
    zenoh::init_log_from_env_or("error");

    let (config, key_expr, payload, attachment, add_matching_listener) = parse_args();

    println!("Opening session...");
    let session = zenoh::open(config).await.unwrap();

    println!("Declaring Publisher on '{key_expr}' with DATA priority...");
    let publisher = session.declare_publisher(&key_expr).await.unwrap();

    println!("Declaring REALTIME Publisher on 'demo/example/zenoh-rs-pub-realtime'...");
    let publisher_realtime = session
        .declare_publisher("demo/example/zenoh-rs-pub-realtime")
        .priority(Priority::RealTime)
        .congestion_control(CongestionControl::Block)
        .await
        .unwrap();

    println!("Declaring Publisher on 'demo/example/zenoh-rs-pub2' with DATA priority...");
    let publisher2 = session.declare_publisher("demo/example/zenoh-rs-pub2").await.unwrap();

    if add_matching_listener {
        publisher
            .matching_listener()
            .callback(|matching_status| {
                if matching_status.matching() {
                    println!("DATA Publisher has matching subscribers.");
                } else {
                    println!("DATA Publisher has NO MORE matching subscribers.");
                }
            })
            .background()
            .await
            .unwrap();

        publisher_realtime
            .matching_listener()
            .callback(|matching_status| {
                if matching_status.matching() {
                    println!("REALTIME Publisher has matching subscribers.");
                } else {
                    println!("REALTIME Publisher has NO MORE matching subscribers.");
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
    let start_time = std::time::Instant::now();
    for idx in 0..u32::MAX {
        tokio::time::sleep(Duration::from_millis(PERIOD_MS)).await;
        let elapsed = start_time.elapsed();

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let now_ns = (now.as_secs() as u128) * 1_000_000_000 + now.subsec_nanos() as u128;

        // DATA priority publisher (pub1)
        // pub1 runs for first 30 seconds, then stops to test RECOVER
        if elapsed < Duration::from_secs(30) {
            let prefix_data = format!("[{:4}] ts_ns={} ", idx, now_ns);
            let payload_len_data = bytes_per_interval.saturating_sub(prefix_data.len());
            let payload_data: String = std::iter::repeat('A').take(payload_len_data).collect();
            let buf_data = prefix_data + &payload_data;

            println!("Putting DATA ('{}': '{}')...", &key_expr, buf_data);
            publisher
                .put(buf_data)
                .encoding(Encoding::TEXT_PLAIN)
                .attachment(attachment.clone())
                .await
                .unwrap();
        } else if elapsed >= Duration::from_secs(30) && elapsed < Duration::from_secs(31) {
            println!("=== pub1 STOPPED at 30s to test RECOVER ===");
        }

        // REALTIME priority publisher
        let prefix_rt = format!("[{:4}] ts_ns={} ", idx, now_ns);
        // let payload_len_rt = bytes_per_interval.saturating_sub(prefix_rt.len());
        // let payload_rt: String = std::iter::repeat('R').take(payload_len_rt).collect();
        // let buf_rt = prefix_rt + &payload_rt;

        println!("Putting REALTIME ('demo/example/zenoh-rs-pub-realtime': '{}')...", prefix_rt);
        publisher_realtime
            .put(prefix_rt)
            .encoding(Encoding::TEXT_PLAIN)
            .attachment(attachment.clone())
            .await
            .unwrap();

        // After 30 seconds, also publish to demo/example/zenoh-rs-pub2
        if elapsed >= Duration::from_secs(30) {
            let prefix_pub2 = format!("[{:4}] ts_ns={} ", idx, now_ns);
            let payload_len_pub2 = bytes_per_interval.saturating_sub(prefix_pub2.len());
            let payload_pub2: String = std::iter::repeat('B').take(payload_len_pub2).collect();
            let buf_pub2 = prefix_pub2 + &payload_pub2;

            println!("Putting DATA2 ('demo/example/zenoh-rs-pub2': '{}')...", buf_pub2);
            publisher2
                .put(buf_pub2)
                .encoding(Encoding::TEXT_PLAIN)
                .attachment(attachment.clone())
                .await
                .unwrap();
        }
    }
}

#[derive(clap::Parser, Clone, PartialEq, Eq, Hash, Debug)]
struct Args {
    #[arg(short, long, default_value = "demo/example/zenoh-rs-pub1")]
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
