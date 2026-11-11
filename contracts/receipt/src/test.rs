use near_sdk::test_utils::VMContextBuilder;
use near_sdk::testing_env;
use near_sdk::AccountId;

use crate::merkle::*;
use crate::types::*;
use crate::ReceiptContract;

fn admin_account() -> AccountId {
    "admin.near".parse().unwrap()
}

fn custodian_account() -> AccountId {
    "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".parse().unwrap()
}

fn verifier_account() -> AccountId {
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".parse().unwrap()
}

fn farmer_account() -> AccountId {
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".parse().unwrap()
}

fn lender_account() -> AccountId {
    "lender.near".parse().unwrap()
}

fn buyer_account() -> AccountId {
    "buyer.near".parse().unwrap()
}

fn operator_account() -> AccountId {
    "operator.near".parse().unwrap()
}

fn set_context(caller: AccountId, attached_yocto: u128, block_timestamp_ns: u64) {
    let mut builder = VMContextBuilder::new();
    builder
        .predecessor_account_id(caller.clone())
        .signer_account_id(caller)
        .attached_deposit(near_sdk::NearToken::from_yoctonear(attached_yocto))
        .block_timestamp(block_timestamp_ns);
    testing_env!(builder.build());
}

#[test]
fn test_leaf_hash_vectors() {
    let json_str = include_str!("../../../test-vectors/leaf_hash.json");
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();

    let leaf_val = &json["leaf"];
    let expected_borsh_hex = json["borsh_hex"].as_str().unwrap();
    let expected_leaf_hash = json["leaf_hash"].as_str().unwrap();

    let leaf: LeafInput = serde_json::from_value(leaf_val.clone()).unwrap();

    let borsh_bytes = leaf_borsh_bytes(&leaf).unwrap();
    assert_eq!(hex::encode(&borsh_bytes), expected_borsh_hex);

    let leaf_hash_bytes = compute_leaf_hash(&leaf).unwrap();
    assert_eq!(hex::encode(leaf_hash_bytes), expected_leaf_hash);

    // Test additional vectors
    if let Some(vectors) = json["vectors"].as_array() {
        for v in vectors {
            let l: LeafInput = serde_json::from_value(v["leaf"].clone()).unwrap();
            let exp_hash = v["leaf_hash"].as_str().unwrap();
            let h = hex::encode(compute_leaf_hash(&l).unwrap());
            assert_eq!(h, exp_hash);
        }
    }

    // Test tree root & proof folding
    let tree = &json["tree"];
    let day = tree["day"].as_u64().unwrap() as u32;
    let leaves = tree["leaves"].as_u64().unwrap() as u32;
    let root_hex = tree["root"].as_str().unwrap();
    let prev_head_hex = tree["prev_head"].as_str().unwrap();
    let expected_head_hex = tree["head"].as_str().unwrap();

    let root_bytes: [u8; 32] = hex::decode(root_hex).unwrap().try_into().unwrap();
    let prev_head_bytes: [u8; 32] = hex::decode(prev_head_hex).unwrap().try_into().unwrap();

    let head_bytes = compute_head(&prev_head_bytes, day, leaves, &root_bytes);
    assert_eq!(hex::encode(head_bytes), expected_head_hex);

    // Verify proofs for vector leaves
    let proofs = tree["proofs"].as_array().unwrap();
    let vectors = json["vectors"].as_array().unwrap();
    for (i, v) in vectors.iter().enumerate() {
        let l: LeafInput = serde_json::from_value(v["leaf"].clone()).unwrap();
        let l_hash = compute_leaf_hash(&l).unwrap();
        let p_strings: Vec<String> = proofs[i]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        let mut p_bytes = Vec::new();
        for p_str in &p_strings {
            let b: [u8; 32] = hex::decode(p_str).unwrap().try_into().unwrap();
            p_bytes.push(b);
        }
        let folded = fold_proof(&l_hash, l.index, &p_bytes);
        assert_eq!(hex::encode(folded), root_hex);
    }
}

#[test]
fn test_contract_end_to_end_flow() {
    let now_sec = 20733 * 86400 + 3600; // on day 20733
    let now_ns = now_sec * 1_000_000_000;

    // 1. Initialize
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());
    assert_eq!(contract.get_admin(), admin_account());
    assert_eq!(contract.data_version(), 3);

    // 2. Admin registers warehouse, custodian, verifier, operator (with 1 yocto)
    set_context(admin_account(), 1, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
    contract.add_custodian("KN_DAWANAU".to_string(), custodian_account());
    contract.add_verifier(verifier_account());
    contract.add_recovery_operator(operator_account());

    assert!(contract.is_warehouse("KN_DAWANAU".to_string()));
    assert!(contract.is_custodian("KN_DAWANAU".to_string(), custodian_account()));
    assert!(contract.is_verifier(verifier_account()));
    assert!(contract.is_recovery_operator(operator_account()));

    // 3. Custodian anchors day 20733
    let json_str = include_str!("../../../test-vectors/leaf_hash.json");
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
    let tree = &json["tree"];
    let day = tree["day"].as_u64().unwrap() as u32;
    let leaves = tree["leaves"].as_u64().unwrap() as u32;
    let root_hex = tree["root"].as_str().unwrap().to_string();

    set_context(custodian_account(), 0, now_ns);
    let head = contract.anchor("KN_DAWANAU".to_string(), day, root_hex, leaves);
    assert_eq!(head, tree["head"].as_str().unwrap());

    let anchor = contract.get_anchor("KN_DAWANAU".to_string()).unwrap();
    assert_eq!(anchor.day, day);
    assert_eq!(anchor.head, head);

    // 4. Propose mint (Balance leaf, index 1)
    let balance_leaf: LeafInput = serde_json::from_value(json["leaf"].clone()).unwrap();
    let proof: Vec<String> = tree["proofs"][1]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();

    let not_after = now_sec + 200;
    set_context(custodian_account(), 0, now_ns);
    let leaf_hash = contract.mint_propose(balance_leaf.clone(), proof, not_after.into());
    assert_eq!(leaf_hash, json["leaf_hash"].as_str().unwrap());

    // 5. Approve mint (Verifier)
    set_context(verifier_account(), 0, now_ns + 10_000_000_000);
    let receipt_id_str = contract.mint_approve(
        "KN_DAWANAU".to_string(),
        balance_leaf.deposit_no.into(),
        leaf_hash,
    );
    assert_eq!(receipt_id_str, "1");
    assert_eq!(contract.last_receipt_id(), "1");
    assert!(contract.is_claimed("KN_DAWANAU".to_string(), balance_leaf.deposit_no.into()));

    let receipt = contract.get_receipt(1.into()).unwrap();
    assert_eq!(receipt.status, Status::Active);
    assert_eq!(receipt.weight_kg, "1995");
    assert_eq!(receipt.bag_count, 20);
    assert_eq!(receipt.owner, farmer_account().to_string());

    // 6. Lender offers lock for partial amount (500 kg out of 1995 kg)
    let loan_due = now_sec + 86400 * 30;
    let grace = 86400 * 7;
    let offer_expiry = now_sec + 86400 * 2;
    set_context(lender_account(), 1, now_ns + 20_000_000_000);
    contract.lock_offer(
        1.into(),
        500.into(),
        "loan-ref-123".to_string(),
        loan_due.into(),
        grace.into(),
        offer_expiry.into(),
    );

    // 7. Owner accepts lock -> splits into taken (receipt 2, locked) and remainder (receipt 1, active)
    set_context(farmer_account(), 0, now_ns + 30_000_000_000);
    let locked_id = contract.lock_accept(
        1.into(),
        lender_account(),
        500.into(),
        "loan-ref-123".to_string(),
        loan_due.into(),
        grace.into(),
    );
    assert_eq!(locked_id, "2");

    let r1 = contract.get_receipt(1.into()).unwrap();
    let r2 = contract.get_receipt(2.into()).unwrap();

    // Weight and bag conservation check
    assert_eq!(r1.weight_kg, "1495");
    assert_eq!(r2.weight_kg, "500");
    assert_eq!(r1.bag_count + r2.bag_count, 20);
    assert_eq!(r1.status, Status::Active);
    assert_eq!(r2.status, Status::Locked);
    assert_eq!(r2.lender, Some(lender_account().to_string()));

    // 8. Extend due date
    let new_due = loan_due + 86400 * 14;
    set_context(lender_account(), 0, now_ns + 40_000_000_000);
    contract.extend_due(2.into(), new_due.into());
    let lock_view = contract.get_lock(2.into()).unwrap();
    assert_eq!(lock_view.due_date, new_due);

    // 9. Start recovery after due date + grace period
    let after_default_sec = new_due + grace + 100;
    let after_default_ns = after_default_sec * 1_000_000_000;
    set_context(lender_account(), 0, after_default_ns);
    contract.start_recovery(2.into());
    let r2_rec = contract.get_receipt(2.into()).unwrap();
    assert_eq!(r2_rec.status, Status::Recovery);

    // 10. Lender approves sale
    let sale_expiry = after_default_sec + 86400 * 3;
    contract.approve_sale(
        2.into(),
        buyer_account(),
        "escrow-sale-ref-456".to_string(),
        sale_expiry.into(),
    );

    // 11. Recovery operator completes recovery
    set_context(operator_account(), 0, after_default_ns + 10_000_000_000);
    let sale_id = contract.complete_recovery(
        2.into(),
        buyer_account(),
        "escrow-sale-ref-456".to_string(),
    );
    assert_eq!(sale_id, "1");

    let r2_after_sale = contract.get_receipt(2.into()).unwrap();
    assert_eq!(r2_after_sale.status, Status::Active);
    assert_eq!(r2_after_sale.owner, buyer_account().to_string());
    assert_eq!(r2_after_sale.lender, None);

    let sale_view = contract.get_sale(1.into()).unwrap();
    assert_eq!(sale_view.buyer, buyer_account().to_string());
    assert_eq!(sale_view.settlement_ref, None);

    // 12. Operator records settlement
    contract.record_settlement(1.into(), "bank-settlement-ref-789".to_string());
    let sale_settled = contract.get_sale(1.into()).unwrap();
    assert_eq!(sale_settled.settlement_ref, Some("bank-settlement-ref-789".to_string()));

    // 13. Buyer requests exit for full amount
    let exit_expiry = after_default_sec + 86400 * 5;
    set_context(buyer_account(), 0, after_default_ns + 20_000_000_000);
    contract.request_exit(2.into(), 500.into(), exit_expiry.into());

    // 14. Custodian performs exit
    set_context(custodian_account(), 0, after_default_ns + 30_000_000_000);
    let exited_id = contract.exit(2.into(), 500.into(), 495.into());
    assert_eq!(exited_id, "2");
    let r2_exited = contract.get_receipt(2.into()).unwrap();
    assert_eq!(r2_exited.status, Status::Exited);
    assert_eq!(r2_exited.exit_weight_kg, Some("495".to_string()));

    // 15. Admin preclaims lot numbers
    set_context(admin_account(), 1, after_default_ns + 40_000_000_000);
    contract.preclaim("KN_DAWANAU".to_string(), vec![101.into(), 102.into(), 103.into()]);
    assert!(contract.is_claimed("KN_DAWANAU".to_string(), 101.into()));
    assert!(contract.is_claimed("KN_DAWANAU".to_string(), 102.into()));
    assert!(contract.is_claimed("KN_DAWANAU".to_string(), 103.into()));
}

#[test]
#[should_panic(expected = "E70:Unauthorized")]
fn test_unauthorized_admin_fails() {
    let now_ns = 1_000_000_000;
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());

    set_context(farmer_account(), 1, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
}

#[test]
#[should_panic(expected = "E70:Unauthorized")]
fn test_admin_missing_deposit_fails() {
    let now_ns = 1_000_000_000;
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());

    // zero attached deposit
    set_context(admin_account(), 0, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
}

#[test]
#[should_panic(expected = "E36:LenderIsOwner")]
fn test_lender_is_owner_rejected() {
    let now_sec = 20733 * 86400;
    let now_ns = now_sec * 1_000_000_000;
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());

    set_context(admin_account(), 1, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
    contract.add_custodian("KN_DAWANAU".to_string(), custodian_account());
    contract.add_verifier(verifier_account());

    let json_str = include_str!("../../../test-vectors/leaf_hash.json");
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
    let tree = &json["tree"];
    let day = tree["day"].as_u64().unwrap() as u32;
    let leaves = tree["leaves"].as_u64().unwrap() as u32;
    let root_hex = tree["root"].as_str().unwrap().to_string();

    set_context(custodian_account(), 0, now_ns);
    contract.anchor("KN_DAWANAU".to_string(), day, root_hex, leaves);

    let balance_leaf: LeafInput = serde_json::from_value(json["leaf"].clone()).unwrap();
    let proof: Vec<String> = tree["proofs"][1]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();

    let not_after = now_sec + 200;
    let leaf_hash = contract.mint_propose(balance_leaf.clone(), proof, not_after.into());

    set_context(verifier_account(), 0, now_ns + 1_000_000_000);
    contract.mint_approve("KN_DAWANAU".to_string(), balance_leaf.deposit_no.into(), leaf_hash);

    // Farmer attempts to lock to himself
    set_context(farmer_account(), 1, now_ns + 2_000_000_000);
    contract.lock_offer(
        1.into(),
        500.into(),
        "loan-self".to_string(),
        (now_sec + 86400 * 30).into(),
        (86400 * 7).into(),
        (now_sec + 86400).into(),
    );
}

#[test]
#[should_panic(expected = "E54:AnchorOutOfOrder")]
fn test_anchor_out_of_order_rejected() {
    let now_sec = 20733 * 86400;
    let now_ns = now_sec * 1_000_000_000;
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());

    set_context(admin_account(), 1, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
    contract.add_custodian("KN_DAWANAU".to_string(), custodian_account());

    let dummy_root = hex::encode([1u8; 32]);
    set_context(custodian_account(), 0, now_ns);
    contract.anchor("KN_DAWANAU".to_string(), 20730, dummy_root.clone(), 1);

    // Attempting same or earlier day
    contract.anchor("KN_DAWANAU".to_string(), 20730, dummy_root, 1);
}

#[test]
#[should_panic(expected = "E72:PendingExpired")]
fn test_expired_mint_propose_rejected() {
    let now_sec = 20733 * 86400;
    let now_ns = now_sec * 1_000_000_000;
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());

    set_context(admin_account(), 1, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
    contract.add_custodian("KN_DAWANAU".to_string(), custodian_account());
    contract.add_verifier(verifier_account());

    let json_str = include_str!("../../../test-vectors/leaf_hash.json");
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
    let tree = &json["tree"];
    let day = tree["day"].as_u64().unwrap() as u32;
    let leaves = tree["leaves"].as_u64().unwrap() as u32;
    let root_hex = tree["root"].as_str().unwrap().to_string();

    set_context(custodian_account(), 0, now_ns);
    contract.anchor("KN_DAWANAU".to_string(), day, root_hex, leaves);

    let balance_leaf: LeafInput = serde_json::from_value(json["leaf"].clone()).unwrap();
    let proof: Vec<String> = tree["proofs"][1]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();

    let not_after = now_sec + 100;
    let leaf_hash = contract.mint_propose(balance_leaf.clone(), proof, not_after.into());

    // Verifier calls AFTER not_after
    let past_deadline_ns = (not_after + 1) * 1_000_000_000;
    set_context(verifier_account(), 0, past_deadline_ns);
    contract.mint_approve("KN_DAWANAU".to_string(), balance_leaf.deposit_no.into(), leaf_hash);
}

#[test]
#[should_panic(expected = "E74:TermsMismatch")]
fn test_lock_accept_terms_mismatch_rejected() {
    let now_sec = 20733 * 86400;
    let now_ns = now_sec * 1_000_000_000;
    set_context(admin_account(), 0, now_ns);
    let mut contract = ReceiptContract::new(admin_account());

    set_context(admin_account(), 1, now_ns);
    contract.add_warehouse("KN_DAWANAU".to_string());
    contract.add_custodian("KN_DAWANAU".to_string(), custodian_account());
    contract.add_verifier(verifier_account());

    let json_str = include_str!("../../../test-vectors/leaf_hash.json");
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
    let tree = &json["tree"];
    let day = tree["day"].as_u64().unwrap() as u32;
    let leaves = tree["leaves"].as_u64().unwrap() as u32;
    let root_hex = tree["root"].as_str().unwrap().to_string();

    set_context(custodian_account(), 0, now_ns);
    contract.anchor("KN_DAWANAU".to_string(), day, root_hex, leaves);

    let balance_leaf: LeafInput = serde_json::from_value(json["leaf"].clone()).unwrap();
    let proof: Vec<String> = tree["proofs"][1]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();

    let not_after = now_sec + 200;
    let leaf_hash = contract.mint_propose(balance_leaf.clone(), proof, not_after.into());

    set_context(verifier_account(), 0, now_ns + 10_000_000_000);
    contract.mint_approve("KN_DAWANAU".to_string(), balance_leaf.deposit_no.into(), leaf_hash);

    let loan_due = now_sec + 86400 * 30;
    let grace = 86400 * 7;
    let offer_expiry = now_sec + 86400 * 2;
    set_context(lender_account(), 1, now_ns + 20_000_000_000);
    contract.lock_offer(
        1.into(),
        500.into(),
        "loan-ref-123".to_string(),
        loan_due.into(),
        grace.into(),
        offer_expiry.into(),
    );

    // Farmer accepts with wrong amount
    set_context(farmer_account(), 0, now_ns + 30_000_000_000);
    contract.lock_accept(
        1.into(),
        lender_account(),
        400.into(), // mismatch!
        "loan-ref-123".to_string(),
        loan_due.into(),
        grace.into(),
    );
}
