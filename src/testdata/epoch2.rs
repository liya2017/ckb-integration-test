use crate::node::{Node, NodeOptions};
use crate::testdata::{dump_testdata, Testdata};
use crate::{CKB2019, CKB2021};

pub struct Epoch2V1TestData;

impl Testdata for Epoch2V1TestData {
    fn generate(&self) {
        let node_options = NodeOptions {
            node_name: "node2019",
            ckb_binary: CKB2019.lock().clone(),
            initial_database: "db/empty",
            chain_spec: "spec/ckb2021",
            app_config: "config/ckb2021",
        };
        let mut node = Node::init(self.testdata_name(), node_options);
        node.start();
        loop {
            node.mine(1);
            let tip = node.get_tip_block();
            if tip.epoch().number() >= 2 {
                break;
            }
        }
        dump_testdata(node, self.testdata_name());
    }
}
pub struct Epoch2V2TestData;

impl Testdata for Epoch2V2TestData {
    fn generate(&self) {
        let node_options = NodeOptions {
            node_name: "node2021",
            ckb_binary: CKB2021.lock().clone(),
            initial_database: "db/empty",
            chain_spec: "spec/ckb2021",
            app_config: "config/ckb2021",
        };
        let mut node = Node::init(self.testdata_name(), node_options);
        node.start();
        loop {
            node.mine(1);
            if node.rpc_client().get_current_epoch().number.value() >= 2 {
                break;
            }
        }
        dump_testdata(node, self.testdata_name());
    }
}
