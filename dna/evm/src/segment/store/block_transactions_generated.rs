// automatically generated by the FlatBuffers compiler, do not modify
// @generated
extern crate alloc;
extern crate flatbuffers;
use self::flatbuffers::{EndianScalar, Follow};
use super::*;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::mem;
pub enum BlockTransactionsOffset {}
#[derive(Copy, Clone, PartialEq)]

pub struct BlockTransactions<'a> {
    pub _tab: flatbuffers::Table<'a>,
}

impl<'a> flatbuffers::Follow<'a> for BlockTransactions<'a> {
    type Inner = BlockTransactions<'a>;
    #[inline]
    unsafe fn follow(buf: &'a [u8], loc: usize) -> Self::Inner {
        Self {
            _tab: flatbuffers::Table::new(buf, loc),
        }
    }
}

impl<'a> BlockTransactions<'a> {
    pub const VT_BLOCK_NUMBER: flatbuffers::VOffsetT = 4;
    pub const VT_TRANSACTIONS: flatbuffers::VOffsetT = 6;

    pub const fn get_fully_qualified_name() -> &'static str {
        "BlockTransactions"
    }

    #[inline]
    pub unsafe fn init_from_table(table: flatbuffers::Table<'a>) -> Self {
        BlockTransactions { _tab: table }
    }
    #[allow(unused_mut)]
    pub fn create<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
        _fbb: &'mut_bldr mut flatbuffers::FlatBufferBuilder<'bldr>,
        args: &'args BlockTransactionsArgs<'args>,
    ) -> flatbuffers::WIPOffset<BlockTransactions<'bldr>> {
        let mut builder = BlockTransactionsBuilder::new(_fbb);
        builder.add_block_number(args.block_number);
        if let Some(x) = args.transactions {
            builder.add_transactions(x);
        }
        builder.finish()
    }

    #[inline]
    pub fn block_number(&self) -> u64 {
        // Safety:
        // Created from valid Table for this object
        // which contains a valid value in this slot
        unsafe {
            self._tab
                .get::<u64>(BlockTransactions::VT_BLOCK_NUMBER, Some(0))
                .unwrap()
        }
    }
    #[inline]
    pub fn transactions(
        &self,
    ) -> Option<flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<Transaction<'a>>>> {
        // Safety:
        // Created from valid Table for this object
        // which contains a valid value in this slot
        unsafe {
            self._tab.get::<flatbuffers::ForwardsUOffset<
                flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<Transaction>>,
            >>(BlockTransactions::VT_TRANSACTIONS, None)
        }
    }
}

impl flatbuffers::Verifiable for BlockTransactions<'_> {
    #[inline]
    fn run_verifier(
        v: &mut flatbuffers::Verifier,
        pos: usize,
    ) -> Result<(), flatbuffers::InvalidFlatbuffer> {
        use self::flatbuffers::Verifiable;
        v.visit_table(pos)?
            .visit_field::<u64>("block_number", Self::VT_BLOCK_NUMBER, false)?
            .visit_field::<flatbuffers::ForwardsUOffset<
                flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<Transaction>>,
            >>("transactions", Self::VT_TRANSACTIONS, false)?
            .finish();
        Ok(())
    }
}
pub struct BlockTransactionsArgs<'a> {
    pub block_number: u64,
    pub transactions: Option<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<Transaction<'a>>>,
        >,
    >,
}
impl<'a> Default for BlockTransactionsArgs<'a> {
    #[inline]
    fn default() -> Self {
        BlockTransactionsArgs {
            block_number: 0,
            transactions: None,
        }
    }
}

pub struct BlockTransactionsBuilder<'a: 'b, 'b> {
    fbb_: &'b mut flatbuffers::FlatBufferBuilder<'a>,
    start_: flatbuffers::WIPOffset<flatbuffers::TableUnfinishedWIPOffset>,
}
impl<'a: 'b, 'b> BlockTransactionsBuilder<'a, 'b> {
    #[inline]
    pub fn add_block_number(&mut self, block_number: u64) {
        self.fbb_
            .push_slot::<u64>(BlockTransactions::VT_BLOCK_NUMBER, block_number, 0);
    }
    #[inline]
    pub fn add_transactions(
        &mut self,
        transactions: flatbuffers::WIPOffset<
            flatbuffers::Vector<'b, flatbuffers::ForwardsUOffset<Transaction<'b>>>,
        >,
    ) {
        self.fbb_.push_slot_always::<flatbuffers::WIPOffset<_>>(
            BlockTransactions::VT_TRANSACTIONS,
            transactions,
        );
    }
    #[inline]
    pub fn new(
        _fbb: &'b mut flatbuffers::FlatBufferBuilder<'a>,
    ) -> BlockTransactionsBuilder<'a, 'b> {
        let start = _fbb.start_table();
        BlockTransactionsBuilder {
            fbb_: _fbb,
            start_: start,
        }
    }
    #[inline]
    pub fn finish(self) -> flatbuffers::WIPOffset<BlockTransactions<'a>> {
        let o = self.fbb_.end_table(self.start_);
        flatbuffers::WIPOffset::new(o.value())
    }
}

impl core::fmt::Debug for BlockTransactions<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut ds = f.debug_struct("BlockTransactions");
        ds.field("block_number", &self.block_number());
        ds.field("transactions", &self.transactions());
        ds.finish()
    }
}
