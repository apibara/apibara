//! # Sequencer
//!
//! The sequencer is used to track input and output sequence numbers.
//! All messages in the Apibara protocol contain a sequence number without
//! gaps. Chain reorganizations create an additional issue: the same sequence
//! number can repeat and all data following a reorged block must be
//! invalidated.
//!
//! The sequencer tracks input sequence numbers from multiple sources and
//! associates them with the output sequence number.
//! The sequencer can invalidate its output sequence numbers in response to
//! an input sequence invalidation.
//!
//! ## Example
//!
//! Imagine a system with three inputs `A`, `B`, and `C`.
//! Each input message is handled by an application that produces zero or more
//! output messages.
//! Notice that in this example is not concerned how input messages are received
//! or how outputs are produced. The sequencer is only involved in tracking and
//! mapping sequence numbers.
//!
//! The first message comes from `A` and has sequence `0`, the application produces
//! two messages. The diagram also includes the state of the input and output sequences.
//!
//! ```txt
//!  IN |A 0|
//!
//! OUT |O 0|O 1|
//!
//! INPUT SEQUENCE
//!   A: 0
//!
//! OUTPUT SEQUENCE: 1
//! ```
//!
//! Then it receives another message from `A`, this time producing a single output.
//!
//! ```txt
//!  IN |A 0|   |A 1|
//!
//! OUT |O 0|O 1|O 2|
//!
//! INPUT SEQUENCE
//!   A: 1
//!
//! OUTPUT SEQUENCE: 2
//! ```
//!
//! After several messages the state of the stream is the following:
//!
//! ```txt
//!  IN |A 0|   |A 1|B 0|B 1|A 2|       |C 0|B 2|
//!
//! OUT |O 0|O 1|O 2|   |O 3|O 4|O 5|O 6|O 7|O 8|O 9|
//!
//! INPUT SEQUENCE
//!   A: 2
//!   B: 2
//!   C: 0
//!
//! OUTPUT SEQUENCE: 9
//! ```
//!
//! Imagine that the stream receives a message invalidating all data produced by
//! `B` after (and including) sequence `1`. This is denoted as `Bx1` in the diagram.
//! The sequencer must rollback its state to just before receiving `B 1` for the first
//! time.
//!
//! ```txt
//!  IN |A 0|   |A 1|B 0|B 1|A 2|       |C 0|B 2|   |Bx1|
//!
//! OUT |O 0|O 1|O 2|   |O 3|O 4|O 5|O 6|O 7|O 8|O 9|
//!
//! INPUT SEQUENCE
//!   A: 1
//!   B: 0
//!
//! OUTPUT SEQUENCE: 2
//! ```
//!
//! Then the stream receives the new message `B'1` and operations resume.
//!
//! ```txt
//!  IN |A 0|   |A 1|B 0|B 1|A 2|       |C 0|B 2|   |Bx1|B'1|
//!
//! OUT |O 0|O 1|O 2|   |O 3|O 4|O 5|O 6|O 7|O 8|O 9|   |O 3|
//!
//! INPUT SEQUENCE
//!   A: 1
//!   B: 1
//!
//! OUTPUT SEQUENCE: 3
//! ```

use std::sync::Arc;

use apibara_core::stream::{Sequence, SequenceRange, StreamId};
use libmdbx::{Environment, EnvironmentKind, Error as MdbxError, Transaction, TransactionKind, RW};

use crate::db::{tables, MdbxRWTransactionExt, MdbxTransactionExt, TableCursor};

pub struct Sequencer<E: EnvironmentKind> {
    db: Arc<Environment<E>>,
}

#[derive(Debug, thiserror::Error)]
pub enum SequencerError {
    #[error("invalid input stream sequence number")]
    InvalidInputSequence { expected: u64, actual: u64 },
    #[error("input sequence number not found")]
    InputSequenceNotFound,
    #[error("invalidate with sequence number greater than current sequence")]
    InvalidInvalidateSequence { current: u64, actual: u64 },
    #[error("error originating from database")]
    Database(#[from] MdbxError),
}

pub type Result<T> = std::result::Result<T, SequencerError>;

impl<E: EnvironmentKind> Sequencer<E> {
    /// Create a new sequencer, persisting data to the given mdbx environment.
    pub fn new(db: Arc<Environment<E>>) -> Result<Self> {
        let txn = db.begin_rw_txn()?;
        txn.ensure_table::<tables::SequencerStateTable>(None)?;
        txn.ensure_table::<tables::StreamStateTable>(None)?;
        txn.commit()?;
        Ok(Sequencer { db })
    }

    /// Register a new input message `(stream_id, sequence)` that generates
    /// `output_len` output messages.
    ///
    /// Returns a sequence range for the output. Notice that if `output_len == 0`, then
    /// the output range is empty.
    pub fn register(
        &mut self,
        stream_id: &StreamId,
        sequence: &Sequence,
        output_len: usize,
    ) -> Result<SequenceRange> {
        let txn = self.db.begin_rw_txn()?;
        let out = self.register_with_txn(stream_id, sequence, output_len, &txn)?;
        txn.commit()?;
        Ok(out)
    }

    /// Same as `register` but using the given [Transaction].
    pub fn register_with_txn(
        &self,
        stream_id: &StreamId,
        sequence: &Sequence,
        output_len: usize,
        txn: &Transaction<RW, E>,
    ) -> Result<SequenceRange> {
        let mut sequencer_cursor = txn.open_table::<tables::SequencerStateTable>()?.cursor()?;
        let mut stream_cursor = txn.open_table::<tables::StreamStateTable>()?.cursor()?;

        // Check the input sequence number is +1 of the previous input's sequence.
        if let Some((_, state)) = stream_cursor.seek_exact(stream_id)? {
            if let Some(input_sequence) = state.sequence {
                if sequence.as_u64() != input_sequence + 1 {
                    return Err(SequencerError::InvalidInputSequence {
                        expected: input_sequence + 1,
                        actual: sequence.as_u64(),
                    });
                }
            }
        }

        // Find the current output sequence. Since all streams state is ordered, only need
        // to check the last item for each stream.
        let output_sequence_start = self
            .output_sequence_start_with_cursor(&mut sequencer_cursor, &mut stream_cursor)?
            .as_u64();

        // Create range of output values.
        let output_len = output_len as u64;
        let output_sequence_end = Sequence::from_u64(output_sequence_start + output_len);
        let output_sequence_start = Sequence::from_u64(output_sequence_start);
        let output_sequence = SequenceRange::new(&output_sequence_start, &output_sequence_end);

        // Update stream state for the current input.
        stream_cursor.seek_exact(stream_id)?;
        let new_stream_state = tables::StreamState {
            sequence: Some(sequence.as_u64()),
        };
        stream_cursor.put(stream_id, &new_stream_state)?;

        // Update sequencer state for the input stream.
        sequencer_cursor.seek_exact(&(*stream_id, *sequence))?;
        let new_sequencer_state = tables::SequencerState {
            output_sequence_start: Some(output_sequence_start.as_u64()),
            output_sequence_end: Some(output_sequence_end.as_u64()),
        };
        sequencer_cursor.put(&(*stream_id, *sequence), &new_sequencer_state)?;

        // Finish updating data.
        txn.commit()?;

        Ok(output_sequence)
    }

    /// Invalidates all messages received after (inclusive) `(stream_id, sequence)`.
    ///
    /// Returns the sequence number of the first invalidated messages of the output stream.
    pub fn invalidate(&mut self, stream_id: &StreamId, sequence: &Sequence) -> Result<Sequence> {
        let txn = self.db.begin_rw_txn()?;
        let mut sequencer_cursor = txn.open_table::<tables::SequencerStateTable>()?.cursor()?;
        let mut stream_cursor = txn.open_table::<tables::StreamStateTable>()?.cursor()?;

        match stream_cursor.seek_exact(stream_id)? {
            None => {
                // The provided stream is not a stream tracked.
                txn.commit()?;
                return Err(SequencerError::InputSequenceNotFound);
            }
            Some((_, state)) => {
                // Check if the invalidated sequence is consistent with the stream state.
                if state.sequence() < sequence.as_u64() {
                    txn.commit()?;
                    return Err(SequencerError::InvalidInvalidateSequence {
                        current: state.sequence(),
                        actual: sequence.as_u64(),
                    });
                }
            }
        }

        let (_, invalidated_input_state) = sequencer_cursor
            .seek_exact(&(*stream_id, *sequence))?
            .ok_or(SequencerError::InputSequenceNotFound)?;
        let first_invalidated_output_sequence_start =
            invalidated_input_state.output_sequence_start();

        // Now invalidate all outputs with output_sequence_start later than
        // the current input's output_sequence_start
        let mut stream = stream_cursor.first()?;
        while let Some((stream_id, stream_state)) = stream {
            // Move to the latest stream sequence state and iterate backwards,
            // deleting now invalidated data.
            let mut sequencer = sequencer_cursor
                .seek_exact(&(stream_id, Sequence::from_u64(stream_state.sequence())))?;
            let mut new_stream_sequence = None;
            while let Some(((sequencer_stream_id, sequencer_sequence), sequencer_state)) = sequencer
            {
                // Moved to another stream. So stop iteration.
                if sequencer_stream_id != stream_id {
                    // Deleted all sequences from this stream.
                    // Delete stream state to keep state consistent so that it
                    // looks like the sequencer never received any data from this
                    // stream.
                    stream_cursor.del()?;
                    // Don't updated state at the end of the loop.
                    new_stream_sequence = None;
                    break;
                }
                new_stream_sequence = Some(sequencer_sequence.as_u64());
                // Here we compare with output_sequence_end since if the input did
                // not generate any value, this value is less than output_sequence_start.
                //
                // If the value being deleted is immediately before the invalidated input,
                // this avoids mistakenly delete the value.
                // If the empty output is _after_ the invalidated input, it's still deleted.
                // output_start is non-inclusive
                if let Some(output_start) = sequencer_state.output_sequence_end {
                    if output_start > first_invalidated_output_sequence_start {
                        sequencer_cursor.del()?;
                    } else {
                        // no need to continue iterating this stream.
                        break;
                    }
                }

                // Advance to previous state
                sequencer = sequencer_cursor.prev()?;
            }

            if let Some(sequence) = new_stream_sequence {
                // update sequencer state
                let new_stream_state = tables::StreamState {
                    sequence: Some(sequence),
                };
                stream_cursor.put(&stream_id, &new_stream_state)?;
            }

            // Advance to next stream.
            stream = stream_cursor.next()?;
        }
        txn.commit()?;
        Ok(Sequence::from_u64(first_invalidated_output_sequence_start))
    }

    /// Returns the start sequence of the next output message.
    pub fn next_output_sequence_start(&self) -> Result<Sequence> {
        let txn = self.db.begin_ro_txn()?;
        let mut sequence_cursor = txn.open_table::<tables::SequencerStateTable>()?.cursor()?;
        let mut stream_cursor = txn.open_table::<tables::StreamStateTable>()?.cursor()?;
        let sequence =
            self.output_sequence_start_with_cursor(&mut sequence_cursor, &mut stream_cursor)?;
        txn.commit()?;
        Ok(sequence)
    }

    /// Returns the latest/current sequence of the given input `stream_id`.
    pub fn input_sequence(&self, stream_id: &StreamId) -> Result<Option<Sequence>> {
        let txn = self.db.begin_ro_txn()?;
        let mut stream_cursor = txn.open_table::<tables::StreamStateTable>()?.cursor()?;
        if let Some((_, state)) = stream_cursor.seek_exact(stream_id)? {
            if let Some(sequence) = state.sequence {
                txn.commit()?;
                return Ok(Some(Sequence::from_u64(sequence)));
            }
        }
        txn.commit()?;
        Ok(None)
    }

    /// Find the current output sequence. Since all streams state is ordered, only need
    /// to check the last item for each stream.
    fn output_sequence_start_with_cursor<'txn, K>(
        &self,
        sequencer_cursor: &mut TableCursor<'txn, tables::SequencerStateTable, K>,
        stream_cursor: &mut TableCursor<'txn, tables::StreamStateTable, K>,
    ) -> Result<Sequence>
    where
        K: TransactionKind,
    {
        let mut output_sequence_start = None;
        let mut stream = stream_cursor.first()?;
        while let Some((stream_id, stream_state)) = stream {
            stream = stream_cursor.next()?;

            let stream_sequence = Sequence::from_u64(stream_state.sequence());
            let (_, sequencer_state) = sequencer_cursor
                .seek_exact(&(stream_id, stream_sequence))?
                .ok_or(SequencerError::InputSequenceNotFound)?;

            if let Some(output_sequence) = sequencer_state.output_sequence_end {
                output_sequence_start = match output_sequence_start {
                    None => Some(output_sequence),
                    Some(curr_output_start) => {
                        // Output sequence start at successor of current value
                        Some(u64::max(curr_output_start, output_sequence))
                    }
                }
            }
        }

        // If no input was found, start at 0.
        Ok(Sequence::from_u64(
            output_sequence_start.unwrap_or_default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use apibara_core::stream::{Sequence, StreamId};
    use libmdbx::{Environment, NoWriteMap};
    use tempfile::tempdir;

    use crate::db::MdbxEnvironmentExt;

    use super::Sequencer;

    #[test]
    pub fn test_sequencer() {
        let path = tempdir().unwrap();
        let db = Environment::<NoWriteMap>::open(path.path()).unwrap();
        let mut sequencer = Sequencer::new(Arc::new(db)).unwrap();

        let s_a = StreamId::from_u64(0);
        let s_b = StreamId::from_u64(1);
        let s_c = StreamId::from_u64(2);

        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 0);

        let output_range = sequencer.register(&s_a, &Sequence::from_u64(0), 2).unwrap();
        assert_eq!(output_range.start().as_u64(), 0);
        assert_eq!(output_range.end().as_u64(), 2);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 2);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 0);
        assert!(sequencer.input_sequence(&s_b).unwrap().is_none());
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());

        let output_range = sequencer.register(&s_a, &Sequence::from_u64(1), 1).unwrap();
        assert_eq!(output_range.start().as_u64(), 2);
        assert_eq!(output_range.end().as_u64(), 3);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 3);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 1);
        assert!(sequencer.input_sequence(&s_b).unwrap().is_none());
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());

        let output_range = sequencer.register(&s_b, &Sequence::from_u64(0), 0).unwrap();
        assert!(output_range.is_empty());
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 3);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 1);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 0);
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());

        let output_range = sequencer.register(&s_b, &Sequence::from_u64(1), 1).unwrap();
        assert_eq!(output_range.start().as_u64(), 3);
        assert_eq!(output_range.end().as_u64(), 4);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 4);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 1);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 1);
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());

        let output_range = sequencer.register(&s_a, &Sequence::from_u64(2), 3).unwrap();
        assert_eq!(output_range.start().as_u64(), 4);
        assert_eq!(output_range.end().as_u64(), 7);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 7);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 2);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 1);
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());

        let output_range = sequencer.register(&s_c, &Sequence::from_u64(0), 1).unwrap();
        assert_eq!(output_range.start().as_u64(), 7);
        assert_eq!(output_range.end().as_u64(), 8);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 8);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 2);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 1);
        assert_eq!(sequencer.input_sequence(&s_c).unwrap().unwrap().as_u64(), 0);

        let output_range = sequencer.register(&s_b, &Sequence::from_u64(2), 2).unwrap();
        assert_eq!(output_range.start().as_u64(), 8);
        assert_eq!(output_range.end().as_u64(), 10);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 10);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 2);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 2);
        assert_eq!(sequencer.input_sequence(&s_c).unwrap().unwrap().as_u64(), 0);

        let invalidated_sequence = sequencer.invalidate(&s_b, &Sequence::from_u64(1)).unwrap();
        assert_eq!(invalidated_sequence.as_u64(), 3);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 3);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 1);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 0);
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());

        let output_range = sequencer.register(&s_b, &Sequence::from_u64(1), 1).unwrap();
        assert_eq!(output_range.start().as_u64(), 3);
        assert_eq!(output_range.end().as_u64(), 4);
        assert_eq!(sequencer.next_output_sequence_start().unwrap().as_u64(), 4);
        assert_eq!(sequencer.input_sequence(&s_a).unwrap().unwrap().as_u64(), 1);
        assert_eq!(sequencer.input_sequence(&s_b).unwrap().unwrap().as_u64(), 1);
        assert!(sequencer.input_sequence(&s_c).unwrap().is_none());
    }
}
