use super::proto::v1alpha2::*;

impl HeaderFilter {
    /// Create an header filter that always matches an header.
    pub fn new() -> Self {
        HeaderFilter { weak: false }
    }

    /// Create an header filter that returns an header only if other filters match.
    pub fn weak() -> Self {
        HeaderFilter { weak: true }
    }
}

impl Filter {
    /// Configure filter header.
    pub fn with_header(&mut self, header: HeaderFilter) -> &mut Self {
        self.header = Some(header);
        self
    }

    /// With specific state update.
    pub fn with_state_update(&mut self, state_udpate: StateUpdateFilter) -> &mut Self {
        self.state_update = Some(state_udpate);
        self
    }

    /// Add event to subscribe to.
    pub fn add_event<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(EventFilter) -> EventFilter,
    {
        self.events.push(closure(EventFilter::default()));
        self
    }

    /// Add transaction to filter.
    pub fn add_transaction<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(TransactionFilter) -> TransactionFilter,
    {
        self.transactions
            .push(closure(TransactionFilter::default()));
        self
    }

    /// Add message to filter.
    pub fn add_message<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(L2ToL1MessageFilter) -> L2ToL1MessageFilter,
    {
        self.messages.push(closure(L2ToL1MessageFilter::default()));
        self
    }

    /// Build final version of Filter
    pub fn build(&mut self) -> Self {
        // As the ::prost::Message already impl Default trait and doesn't seems to be overridable
        // this a workaround to set the default value.
        // HeaderFilter needs to be set to a value in order to correctly stream data
        if self.header.is_none() {
            self.header = Some(HeaderFilter::weak());
        }
        self.clone()
    }
}

impl TransactionFilter {
    /// Create `InvokeTransactionV0Filter` from `TransactionFilter`
    pub fn invoke_transaction_v0<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(InvokeTransactionV0Filter) -> InvokeTransactionV0Filter,
    {
        self.filter = Some(transaction_filter::Filter::InvokeV0(closure(
            InvokeTransactionV0Filter::default(),
        )));
        self
    }

    /// Create `InvokeTransactionV1Filter` from `TransactionFilter`
    pub fn invoke_transaction_v1<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(InvokeTransactionV1Filter) -> InvokeTransactionV1Filter,
    {
        self.filter = Some(transaction_filter::Filter::InvokeV1(closure(
            InvokeTransactionV1Filter::default(),
        )));
        self
    }

    /// Create `DeployTransactionFilter` from `TransactionFilter`
    pub fn deploy_transaction<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(DeployTransactionFilter) -> DeployTransactionFilter,
    {
        self.filter = Some(transaction_filter::Filter::Deploy(closure(
            DeployTransactionFilter::default(),
        )));
        self
    }

    /// Create `DeclareTransactionFilter` from `TransactionFilter`
    pub fn declare_transaction<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(DeclareTransactionFilter) -> DeclareTransactionFilter,
    {
        self.filter = Some(transaction_filter::Filter::Declare(closure(
            DeclareTransactionFilter::default(),
        )));
        self
    }

    /// Create `L1HandlerTransactionFilter` from `TransactionFilter`
    pub fn l1_handler_transaction<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(L1HandlerTransactionFilter) -> L1HandlerTransactionFilter,
    {
        self.filter = Some(transaction_filter::Filter::L1Handler(closure(
            L1HandlerTransactionFilter::default(),
        )));
        self
    }

    /// Create `DeployAccountTransactionFilter` from `TransactionFilter`
    pub fn deploy_account_transaction<F>(&mut self, closure: F) -> &mut Self
    where
        F: Fn(DeployAccountTransactionFilter) -> DeployAccountTransactionFilter,
    {
        self.filter = Some(transaction_filter::Filter::DeployAccount(closure(
            DeployAccountTransactionFilter::default(),
        )));
        self
    }

    /// Builds final `TransactionFilter`
    pub fn build(&mut self) -> Self {
        self.clone()
    }
}

impl InvokeTransactionV0Filter {
    /// Filter transaction with contract address.
    pub fn with_contract_address(mut self, address: FieldElement) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Filter with transaction selector.
    pub fn with_entry_point_selector(mut self, selector: FieldElement) -> Self {
        self.entry_point_selector = Some(selector);
        self
    }

    /// Filter with call data.
    pub fn with_calldata(mut self, calldata: Vec<FieldElement>) -> Self {
        self.calldata = calldata;
        self
    }
}

impl InvokeTransactionV1Filter {
    /// Filter transaction with sender address.
    pub fn with_sender_address(mut self, address: FieldElement) -> Self {
        self.sender_address = Some(address);
        self
    }

    /// Filter with call data.
    pub fn with_calldata(mut self, calldata: Vec<FieldElement>) -> Self {
        self.calldata = calldata;
        self
    }
}

impl DeployTransactionFilter {
    /// Filter transaction with contract address salt.
    pub fn with_contract_address_salt(mut self, address: FieldElement) -> Self {
        self.contract_address_salt = Some(address);
        self
    }
    /// Filter transaction with class hash.
    pub fn with_class_hash(mut self, class_hash: FieldElement) -> Self {
        self.class_hash = Some(class_hash);
        self
    }

    /// Filter transaction with constructor calldata.
    pub fn with_constructor_calldata(mut self, constructor_calldata: Vec<FieldElement>) -> Self {
        self.constructor_calldata = constructor_calldata;
        self
    }
}

impl DeclareTransactionFilter {
    /// Filter transaction with sender address.
    pub fn with_sender_address(mut self, address: FieldElement) -> Self {
        self.sender_address = Some(address);
        self
    }

    /// Filter with class hash.
    pub fn with_class_hash(mut self, class_hash: FieldElement) -> Self {
        self.class_hash = Some(class_hash);
        self
    }
}

impl L1HandlerTransactionFilter {
    /// Filter transaction with contract address.
    pub fn with_contract_address(mut self, address: FieldElement) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Filter transaction with entry point selector.
    pub fn with_entry_point_selector(mut self, selector: FieldElement) -> Self {
        self.entry_point_selector = Some(selector);
        self
    }

    /// Filter transaction with call data.
    pub fn with_calldata(mut self, calldata: Vec<FieldElement>) -> Self {
        self.calldata = calldata;
        self
    }
}

impl DeployAccountTransactionFilter {
    /// Filter transaction with contract address salt.
    pub fn with_contract_address_salt(mut self, address: FieldElement) -> Self {
        self.contract_address_salt = Some(address);
        self
    }

    /// Filter transaction with class hash.
    pub fn with_class_hash(mut self, class_hash: FieldElement) -> Self {
        self.class_hash = Some(class_hash);
        self
    }

    /// Filter transaction with calldata.
    pub fn with_constructor_calldata(mut self, constructor_calldata: Vec<FieldElement>) -> Self {
        self.constructor_calldata = constructor_calldata;
        self
    }
}

impl EventFilter {
    /// Filter event from address.
    pub fn with_from_address(mut self, address: FieldElement) -> Self {
        self.from_address = Some(address);
        self
    }

    /// Filter event with key.
    pub fn with_keys(mut self, keys: Vec<FieldElement>) -> Self {
        self.keys = keys;
        self
    }

    /// Filter event with data.
    pub fn with_data(mut self, data: Vec<FieldElement>) -> Self {
        self.data = data;
        self
    }
}

impl L2ToL1MessageFilter {
    /// Filter message to address.
    pub fn with_to_address(mut self, to: FieldElement) -> Self {
        self.to_address = Some(to);
        self
    }

    /// Filter message with payload.
    pub fn with_payload(mut self, payload: Vec<FieldElement>) -> Self {
        self.payload = payload;
        self
    }
}

impl StateUpdateFilter {
    /// Add storage diff filter to state update filter.
    pub fn add_storage_diff<F>(mut self, closure: F) -> Self
    where
        F: Fn(StorageDiffFilter) -> StorageDiffFilter,
    {
        self.storage_diffs
            .push(closure(StorageDiffFilter::default()));
        self
    }

    /// Add declared contract filter to state update.
    pub fn add_declared_contract<F>(mut self, closure: F) -> Self
    where
        F: Fn(DeclaredContractFilter) -> DeclaredContractFilter,
    {
        self.declared_contracts
            .push(closure(DeclaredContractFilter::default()));
        self
    }

    /// Add deployed contract filter to state update.
    pub fn add_deployed_contract<F>(mut self, closure: F) -> Self
    where
        F: Fn(DeployedContractFilter) -> DeployedContractFilter,
    {
        self.deployed_contracts
            .push(closure(DeployedContractFilter::default()));
        self
    }

    /// Add nonce update filter to state update.
    pub fn add_nonce_update<F>(mut self, closure: F) -> Self
    where
        F: Fn(NonceUpdateFilter) -> NonceUpdateFilter,
    {
        self.nonces.push(closure(NonceUpdateFilter::default()));
        self
    }
}

impl StorageDiffFilter {
    /// Filter with contract address.
    pub fn with_contract_address(mut self, address: FieldElement) -> Self {
        self.contract_address = Some(address);
        self
    }
}

impl DeclaredContractFilter {
    /// Filter with class hash.
    pub fn with_class_hash(mut self, address: FieldElement) -> Self {
        self.class_hash = Some(address);
        self
    }
}

impl DeployedContractFilter {
    /// Filter with contract address.
    pub fn with_contract_address(mut self, address: FieldElement) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Filter with class hash.
    pub fn with_class_hash(mut self, address: FieldElement) -> Self {
        self.class_hash = Some(address);
        self
    }
}

impl NonceUpdateFilter {
    /// Filter with contract address.
    pub fn with_contract_address(mut self, address: FieldElement) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Filter with nonce.
    pub fn with_nonce(mut self, nonce: FieldElement) -> Self {
        self.nonce = Some(nonce);
        self
    }
}

trait VecMatch {
    fn prefix_matches(&self, other: &Self) -> bool;
}

impl<T> VecMatch for Vec<T>
where
    T: PartialEq,
{
    fn prefix_matches(&self, other: &Self) -> bool {
        if self.is_empty() {
            return true;
        }

        if self.len() > other.len() {
            return false;
        }

        for (a, b) in self.iter().zip(other) {
            if a != b {
                return false;
            }
        }

        true
    }
}

/// [Option] extension trait to match values. `None` matches anything.
trait FilterMatch {
    fn matches(&self, other: &Self) -> bool;
}

impl FilterMatch for Option<FieldElement> {
    fn matches(&self, other: &Self) -> bool {
        if self.is_none() {
            return true;
        }
        self == other
    }
}

impl TransactionFilter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match self.filter.as_ref() {
            None => true,
            Some(transaction_filter::Filter::InvokeV0(filter)) => filter.matches(tx),
            Some(transaction_filter::Filter::InvokeV1(filter)) => filter.matches(tx),
            Some(transaction_filter::Filter::Deploy(filter)) => filter.matches(tx),
            Some(transaction_filter::Filter::Declare(filter)) => filter.matches(tx),
            Some(transaction_filter::Filter::L1Handler(filter)) => filter.matches(tx),
            Some(transaction_filter::Filter::DeployAccount(filter)) => filter.matches(tx),
        }
    }
}

impl InvokeTransactionV0Filter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match tx.transaction.as_ref() {
            Some(transaction::Transaction::InvokeV0(tx)) => {
                self.contract_address.matches(&tx.contract_address)
                    && self.entry_point_selector.matches(&tx.entry_point_selector)
                    && self.calldata.prefix_matches(&tx.calldata)
            }
            _ => false,
        }
    }
}

impl InvokeTransactionV1Filter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match tx.transaction.as_ref() {
            Some(transaction::Transaction::InvokeV1(tx)) => {
                self.sender_address.matches(&tx.sender_address)
                    && self.calldata.prefix_matches(&tx.calldata)
            }
            _ => false,
        }
    }
}

impl DeployTransactionFilter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match tx.transaction.as_ref() {
            Some(transaction::Transaction::Deploy(tx)) => {
                self.class_hash.matches(&tx.class_hash)
                    && self
                        .contract_address_salt
                        .matches(&tx.contract_address_salt)
                    && self
                        .constructor_calldata
                        .prefix_matches(&tx.constructor_calldata)
            }
            _ => false,
        }
    }
}

impl DeclareTransactionFilter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match tx.transaction.as_ref() {
            Some(transaction::Transaction::Declare(tx)) => {
                self.class_hash.matches(&tx.class_hash)
                    && self.sender_address.matches(&tx.sender_address)
            }
            _ => false,
        }
    }
}

impl L1HandlerTransactionFilter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match tx.transaction.as_ref() {
            Some(transaction::Transaction::L1Handler(tx)) => {
                self.contract_address.matches(&tx.contract_address)
                    && self.entry_point_selector.matches(&tx.entry_point_selector)
                    && self.calldata.prefix_matches(&tx.calldata)
            }
            _ => false,
        }
    }
}

impl DeployAccountTransactionFilter {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match tx.transaction.as_ref() {
            Some(transaction::Transaction::DeployAccount(tx)) => {
                self.class_hash.matches(&tx.class_hash)
                    && self
                        .contract_address_salt
                        .matches(&tx.contract_address_salt)
                    && self
                        .constructor_calldata
                        .prefix_matches(&tx.constructor_calldata)
            }
            _ => false,
        }
    }
}

impl EventFilter {
    pub fn matches(&self, event: &Event) -> bool {
        self.from_address.matches(&event.from_address)
            && self.keys.prefix_matches(&event.keys)
            && self.data.prefix_matches(&event.data)
    }
}

impl L2ToL1MessageFilter {
    pub fn matches(&self, message: &L2ToL1Message) -> bool {
        self.to_address.matches(&message.to_address)
            && self.payload.prefix_matches(&message.payload)
    }
}

impl StorageDiffFilter {
    pub fn matches(&self, storage_diff: &StorageDiff) -> bool {
        self.contract_address
            .matches(&storage_diff.contract_address)
    }
}

impl DeclaredContractFilter {
    pub fn matches(&self, declared_contract: &DeclaredContract) -> bool {
        self.class_hash.matches(&declared_contract.class_hash)
    }
}

impl DeployedContractFilter {
    pub fn matches(&self, deployed_contract: &DeployedContract) -> bool {
        self.contract_address
            .matches(&deployed_contract.contract_address)
            && self.class_hash.matches(&deployed_contract.class_hash)
    }
}

impl NonceUpdateFilter {
    pub fn matches(&self, nonce: &NonceUpdate) -> bool {
        self.contract_address.matches(&nonce.contract_address) && self.nonce.matches(&nonce.nonce)
    }
}
