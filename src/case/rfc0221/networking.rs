use crate::case::rfc0221::util::{committed_timestamp, median_timestamp};
use crate::case::{Case, CaseOptions};
use crate::node::{Node, NodeOptions};
use crate::nodes::Nodes;
use crate::util::since_from_relative_timestamp;
use crate::{CKB2019, CKB2021};
use ckb_types::core::EpochNumber;
use ckb_types::{
    core::TransactionBuilder,
    packed::{CellInput, CellOutput},
    prelude::*,
};
use std::thread::sleep;
use std::time::Duration;

const RFC0221_EPOCH_NUMBER: EpochNumber = 3;

pub struct RFC0221Networking;

impl Case for RFC0221Networking {
    fn case_options(&self) -> CaseOptions {
        CaseOptions {
            make_all_nodes_connected: true,
            make_all_nodes_synced: true,
            make_all_nodes_connected_and_synced: true,
            node_options: vec![
                NodeOptions {
                    node_name: "node2019",
                    ckb_binary: CKB2019.lock().clone(),
                    initial_database: "db/Epoch2V1TestData",
                    chain_spec: "spec/ckb2021",
                    app_config: "config/ckb2021",
                },
                NodeOptions {
                    node_name: "node2021",
                    ckb_binary: CKB2021.lock().clone(),
                    initial_database: "db/empty",
                    chain_spec: "spec/ckb2021",
                    app_config: "config/ckb2021",
                },
            ]
            .into_iter()
            .collect(),
        }
    }

    fn run(&self, nodes: Nodes) {
        let node2019 = nodes.get_node("node2019");
        let node2021 = nodes.get_node("node2021");

        // Move the chain to height = rfc0221_switch + 37
        {
            let mut over_move_switch_cnt = 37;
            loop {
                if !is_rfc0221_switched(node2021) {
                    node2021.mine(1);
                } else if over_move_switch_cnt > 0 {
                    over_move_switch_cnt -= 1;
                    node2021.mine(1);
                    sleep(Duration::from_secs(1));
                } else {
                    break;
                }
            }
        }

        // Construct a transaction tx:
        //   - since: relative 2 seconds
        let relative_secs = 2;
        let relative_mills = relative_secs * 1000;
        let since = since_from_relative_timestamp(relative_secs);
        let input = &{
            // Use the last live cell as input to make sure the constructed
            // transaction cannot pass the "since verification" at short future
            node2021
                .get_live_always_success_cells()
                .pop()
                .expect("last live cell")
        };
        let input_block_number = input
            .transaction_info
            .as_ref()
            .expect("live cell should have transaction info")
            .block_number;
        let start_time_of_rfc0221 = committed_timestamp(node2021, input_block_number);
        let start_time_of_old_rule = median_timestamp(node2021, input_block_number);
        assert!(start_time_of_old_rule < start_time_of_rfc0221);
        let tx = TransactionBuilder::default()
            .input(CellInput::new(input.out_point.clone(), since))
            .output(
                CellOutput::new_builder()
                    .lock(input.cell_output.lock())
                    .type_(input.cell_output.type_())
                    .capacity(input.capacity().pack())
                    .build(),
            )
            .output_data(Default::default())
            .cell_dep(node2021.always_success_cell_dep())
            .build();

        loop {
            let tip_number = node2021.get_tip_block_number();
            let tip_median_time = median_timestamp(node2021, tip_number);
            assert!(start_time_of_rfc0221 + relative_mills > tip_median_time);
            if start_time_of_old_rule + relative_mills <= tip_median_time {
                break;
            }

            node2021.mine(1);
        }

        // node2021 send tx, fail
        // node2019 send tx, success
        nodes
            .waiting_for_sync()
            .expect("nodes can sync before rfc0221 block");
        let result = node2021
            .rpc_client()
            .send_transaction_result(tx.pack().data().into());
        assert!(
            result.is_err(),
            "After RFC0221, node2021 should reject tx according to rfc0221, but got: {:?}",
            result,
        );
        let sent = node2019
            .rpc_client()
            .send_transaction_result(tx.pack().data().into());
        assert!(
            sent.is_ok(),
            "node2019 should accept tx according to old rule, but got: {:?}",
            sent,
        );

        // node2019 continue to mine 3 blocks
        // node2021 cannot sync as packed tx is invalid
        // node2021 ban node2019
        node2019.mine(node2019.consensus().tx_proposal_window.closest.value() + 1);
        nodes
            .waiting_for_sync()
            .expect_err("nodes cannot sync after rfc0221 block");
        let banned = node2021
            .rpc_client()
            .get_banned_addresses()
            .iter()
            .any(|address| address.ban_reason.contains("BlockIsInvalid(401)"));
        assert!(
            banned,
            "node2021 should ban node2019 after rfc0221 block, but got: {:?}",
            node2021.rpc_client().get_banned_addresses(),
        );
    }
}

fn is_rfc0221_switched(node: &Node) -> bool {
    node.rpc_client().get_current_epoch().number.value() >= RFC0221_EPOCH_NUMBER
}
