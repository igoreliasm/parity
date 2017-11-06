mod helpers;
mod tx_builder;

use self::helpers::{DummyScoring, NonceReady};
use self::tx_builder::TransactionBuilder;

use super::*;

type TestPool = Pool<DummyScoring>;

#[test]
fn should_clear_queue() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();
	assert_eq!(txq.light_status(), LightStatus {
		mem_usage: 0,
		count: 0,
		senders: 0,
	});
	let tx1 = b.tx().nonce(0).new();
	let tx2 = b.tx().nonce(1).new();

	// add
	txq.import(tx1).unwrap();
	txq.import(tx2).unwrap();
	assert_eq!(txq.light_status(), LightStatus {
		mem_usage: 1,
		count: 2,
		senders: 1,
	});

	// when
	txq.clear();

	// then
	assert_eq!(txq.light_status(), LightStatus {
		mem_usage: 0,
		count: 0,
		senders: 0,
	});
}

#[test]
fn should_not_allow_same_transaction_twice() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();
	let tx1 = b.tx().nonce(0).new();
	let tx2 = b.tx().nonce(0).new();

	// when
	txq.import(tx1).unwrap();
	txq.import(tx2).unwrap_err();

	// then
	assert_eq!(txq.light_status().count, 1);
}

#[test]
fn should_replace_transaction() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();
	let tx1 = b.tx().nonce(0).gas_price(1).new();
	let tx2 = b.tx().nonce(0).gas_price(2).new();

	// when
	txq.import(tx1).unwrap();
	txq.import(tx2).unwrap();

	// then
	assert_eq!(txq.light_status().count, 1);
}

#[test]
fn should_reject_if_above_count() {
	let b = TransactionBuilder::default();
	let mut txq = TestPool::with_options(Options {
		max_count: 1,
		..Default::default()
	});

	// Reject second
	let tx1 = b.tx().nonce(0).new();
	let tx2 = b.tx().nonce(1).new();
	let hash = tx2.hash();
	txq.import(tx1).unwrap();
	assert_eq!(txq.import(tx2).unwrap_err().kind(), &error::ErrorKind::TooCheapToEnter(hash));
	assert_eq!(txq.light_status().count, 1);

	txq.clear();

	// Replace first
	let tx1 = b.tx().nonce(0).new();
	let tx2 = b.tx().nonce(0).sender(1).gas_price(2).new();
	txq.import(tx1).unwrap();
	txq.import(tx2).unwrap();
	assert_eq!(txq.light_status().count, 1);
}

#[test]
fn should_reject_if_above_mem_usage() {
	let b = TransactionBuilder::default();
	let mut txq = TestPool::with_options(Options {
		max_mem_usage: 1,
		..Default::default()
	});

	// Reject second
	let tx1 = b.tx().nonce(1).new();
	let tx2 = b.tx().nonce(2).new();
	let hash = tx2.hash();
	txq.import(tx1).unwrap();
	assert_eq!(txq.import(tx2).unwrap_err().kind(), &error::ErrorKind::TooCheapToEnter(hash));
	assert_eq!(txq.light_status().count, 1);

	txq.clear();

	// Replace first
	let tx1 = b.tx().nonce(1).new();
	let tx2 = b.tx().nonce(1).sender(1).gas_price(2).new();
	txq.import(tx1).unwrap();
	txq.import(tx2).unwrap();
	assert_eq!(txq.light_status().count, 1);
}

#[test]
fn should_reject_if_above_sender_count() {
	let b = TransactionBuilder::default();
	let mut txq = TestPool::with_options(Options {
		max_per_sender: 1,
		..Default::default()
	});

	// Reject second
	let tx1 = b.tx().nonce(1).new();
	let tx2 = b.tx().nonce(2).new();
	let hash = tx2.hash();
	txq.import(tx1).unwrap();
	assert_eq!(txq.import(tx2).unwrap_err().kind(), &error::ErrorKind::TooCheapToEnter(hash));
	assert_eq!(txq.light_status().count, 1);

	txq.clear();

	// Replace first
	let tx1 = b.tx().nonce(1).new();
	let tx2 = b.tx().nonce(2).gas_price(2).new();
	let hash = tx2.hash();
	txq.import(tx1).unwrap();
	// This results in error because we also compare nonces
	assert_eq!(txq.import(tx2).unwrap_err().kind(), &error::ErrorKind::TooCheapToEnter(hash));
	assert_eq!(txq.light_status().count, 1);
}

#[test]
fn should_construct_pending() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();

	let tx0 = txq.import(b.tx().nonce(0).gas_price(5).new()).unwrap();
	let tx1 = txq.import(b.tx().nonce(1).gas_price(5).new()).unwrap();
	let tx2 = txq.import(b.tx().nonce(2).new()).unwrap();
	// this transaction doesn't get to the block despite high gas price
	// because of block gas limit and simplistic ordering algorithm.
	txq.import(b.tx().nonce(3).gas_price(4).new()).unwrap();
	//gap
	txq.import(b.tx().nonce(5).new()).unwrap();

	let tx5 = txq.import(b.tx().sender(1).nonce(0).new()).unwrap();
	let tx6 = txq.import(b.tx().sender(1).nonce(1).new()).unwrap();
	let tx7 = txq.import(b.tx().sender(1).nonce(2).new()).unwrap();
	let tx8 = txq.import(b.tx().sender(1).nonce(3).gas_price(4).new()).unwrap();
	// gap
	txq.import(b.tx().sender(1).nonce(5).new()).unwrap();

	let tx9 = txq.import(b.tx().sender(2).nonce(0).new()).unwrap();
	assert_eq!(txq.light_status().count, 11);
	assert_eq!(txq.status(NonceReady::default()), Status {
		stalled: 0,
		pending: 9,
		future: 2,
		senders: 3,
	});
	assert_eq!(txq.status(NonceReady::new(1)), Status {
		stalled: 3,
		pending: 6,
		future: 2,
		senders: 3,
	});

	// when
	let mut current_gas = U256::zero();
	let limit = (21_000 * 8).into();
	let mut pending = txq.pending(NonceReady::default()).take_while(|tx| {
		let should_take = tx.gas + current_gas <= limit;
		if should_take {
			current_gas = current_gas + tx.gas
		}
		should_take
	});

	assert_eq!(pending.next(), Some(tx0));
	assert_eq!(pending.next(), Some(tx1));
	assert_eq!(pending.next(), Some(tx9));
	assert_eq!(pending.next(), Some(tx5));
	assert_eq!(pending.next(), Some(tx6));
	assert_eq!(pending.next(), Some(tx7));
	assert_eq!(pending.next(), Some(tx8));
	assert_eq!(pending.next(), Some(tx2));
	assert_eq!(pending.next(), None);
}

#[test]
fn should_remove_transaction() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();

	let tx1 = txq.import(b.tx().nonce(0).new()).unwrap();
	let tx2 = txq.import(b.tx().nonce(1).new()).unwrap();
	txq.import(b.tx().nonce(2).new()).unwrap();
	assert_eq!(txq.light_status().count, 3);

	// when
	assert!(txq.remove(&tx2.hash(), false));

	// then
	assert_eq!(txq.light_status().count, 2);
	let mut pending = txq.pending(NonceReady::default());
	assert_eq!(pending.next(), Some(tx1));
	assert_eq!(pending.next(), None);
}

#[test]
fn should_cull_stalled_transactions() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();

	txq.import(b.tx().nonce(0).gas_price(5).new()).unwrap();
	txq.import(b.tx().nonce(1).new()).unwrap();
	txq.import(b.tx().nonce(3).new()).unwrap();

	txq.import(b.tx().sender(1).nonce(0).new()).unwrap();
	txq.import(b.tx().sender(1).nonce(1).new()).unwrap();
	txq.import(b.tx().sender(1).nonce(5).new()).unwrap();

	assert_eq!(txq.status(NonceReady::new(1)), Status {
		stalled: 2,
		pending: 2,
		future: 2,
		senders: 2,
	});

	// when
	assert_eq!(txq.cull(None, NonceReady::new(1)), 2);

	// then
	assert_eq!(txq.status(NonceReady::new(1)), Status {
		stalled: 0,
		pending: 2,
		future: 2,
		senders: 2,
	});
}

#[test]
fn should_cull_stalled_transactions_from_a_sender() {
	// given
	let b = TransactionBuilder::default();
	let mut txq = TestPool::default();

	txq.import(b.tx().nonce(0).gas_price(5).new()).unwrap();
	txq.import(b.tx().nonce(1).new()).unwrap();

	txq.import(b.tx().sender(1).nonce(0).new()).unwrap();
	txq.import(b.tx().sender(1).nonce(1).new()).unwrap();
	txq.import(b.tx().sender(1).nonce(2).new()).unwrap();

	assert_eq!(txq.status(NonceReady::new(2)), Status {
		stalled: 4,
		pending: 1,
		future: 0,
		senders: 2,
	});

	// when
	let sender = 0.into();
	assert_eq!(txq.cull(Some(&[sender]), NonceReady::new(2)), 2);

	// then
	assert_eq!(txq.status(NonceReady::new(2)), Status {
		stalled: 2,
		pending: 1,
		future: 0,
		senders: 1,
	});
}

mod listener {
	use super::*;
	use std::rc::Rc;
	use std::cell::RefCell;

	#[derive(Default)]
	struct MyListener(pub Rc<RefCell<Vec<&'static str>>>);

	impl Listener for MyListener {
		fn added(&mut self, _tx: &SharedTransaction, old: Option<&SharedTransaction>) {
			self.0.borrow_mut().push(if old.is_some() { "replaced" } else { "added" });
		}

		fn rejected(&mut self, _tx: VerifiedTransaction) {
			self.0.borrow_mut().push("rejected".into());
		}

		fn dropped(&mut self, _tx: &SharedTransaction) {
			self.0.borrow_mut().push("dropped".into());
		}

		fn invalid(&mut self, _tx: &SharedTransaction) {
			self.0.borrow_mut().push("invalid".into());
		}

		fn cancelled(&mut self, _tx: &SharedTransaction) {
			self.0.borrow_mut().push("cancelled".into());
		}

		fn mined(&mut self, _tx: &SharedTransaction) {
			self.0.borrow_mut().push("mined".into());
		}
	}

	#[test]
	fn insert_transaction() {
		let b = TransactionBuilder::default();
		let listener = MyListener::default();
		let results = listener.0.clone();
		let mut txq = Pool::new(listener, DummyScoring, Options {
			max_per_sender: 1,
			max_count: 2,
			..Default::default()
		});
		assert!(results.borrow().is_empty());

		// Regular import
		txq.import(b.tx().nonce(1).new()).unwrap();
		assert_eq!(*results.borrow(), &["added"]);
		// Already present (no notification)
		txq.import(b.tx().nonce(1).new()).unwrap_err();
		assert_eq!(*results.borrow(), &["added"]);
		// Push out the first one
		txq.import(b.tx().nonce(1).gas_price(1).new()).unwrap();
		assert_eq!(*results.borrow(), &["added", "replaced"]);
		// Reject
		txq.import(b.tx().nonce(1).new()).unwrap_err();
		assert_eq!(*results.borrow(), &["added", "replaced", "rejected"]);
		results.borrow_mut().clear();
		// Different sender (accept)
		txq.import(b.tx().sender(1).nonce(1).gas_price(2).new()).unwrap();
		assert_eq!(*results.borrow(), &["added"]);
		// Third sender push out low gas price
		txq.import(b.tx().sender(2).nonce(1).gas_price(4).new()).unwrap();
		assert_eq!(*results.borrow(), &["added", "dropped", "added"]);
		// Reject (too cheap)
		txq.import(b.tx().sender(2).nonce(1).gas_price(2).new()).unwrap_err();
		assert_eq!(*results.borrow(), &["added", "dropped", "added", "rejected"]);

		assert_eq!(txq.light_status().count, 2);
	}

	#[test]
	fn remove_transaction() {
		let b = TransactionBuilder::default();
		let listener = MyListener::default();
		let results = listener.0.clone();
		let mut txq = Pool::new(listener, DummyScoring, Options::default());

		// insert
		let tx1 = txq.import(b.tx().nonce(1).new()).unwrap();
		let tx2 = txq.import(b.tx().nonce(2).new()).unwrap();

		// then
		txq.remove(&tx1.hash(), false);
		assert_eq!(*results.borrow(), &["added", "added", "cancelled"]);
		txq.remove(&tx2.hash(), true);
		assert_eq!(*results.borrow(), &["added", "added", "cancelled", "invalid"]);
		assert_eq!(txq.light_status().count, 0);
	}

	#[test]
	fn clear_queue() {
		let b = TransactionBuilder::default();
		let listener = MyListener::default();
		let results = listener.0.clone();
		let mut txq = Pool::new(listener, DummyScoring, Options::default());

		// insert
		txq.import(b.tx().nonce(1).new()).unwrap();
		txq.import(b.tx().nonce(2).new()).unwrap();

		// when
		txq.clear();

		// then
		assert_eq!(*results.borrow(), &["added", "added", "dropped", "dropped"]);
	}

	#[test]
	fn cull_stalled() {
		let b = TransactionBuilder::default();
		let listener = MyListener::default();
		let results = listener.0.clone();
		let mut txq = Pool::new(listener, DummyScoring, Options::default());

		// insert
		txq.import(b.tx().nonce(1).new()).unwrap();
		txq.import(b.tx().nonce(2).new()).unwrap();

		// when
		txq.cull(None, NonceReady::new(3));

		// then
		assert_eq!(*results.borrow(), &["added", "added", "mined", "mined"]);
	}
}
