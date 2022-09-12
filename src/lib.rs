//! # A crate for traceable math operations
//! the point of this crate is to turn your mostly regular-looking functions into a nice, pretty
//! graph that shows the flow of data through the computation. It relies a good deal on operator
//! overloading to make everything feel like working with regular floating point numbers, while
//! building a compute graph in the background.

use derivative::Derivative;
use std::{borrow::Cow, fmt::Debug, iter::once};

mod macros;
#[cfg(test)]
mod testing;
mod visualization;

pub(crate) type OpTuple<'a, Num, R> = (&'a Operation<'a, Num>, R);
type History<'a, Num> = Vec<&'a Operation<'a, Num>>;
type OpArena<'a, Num> = typed_arena::Arena<Operation<'a, Num>>;

/// The base arithmetic tracking type. Doing math on this builds a data flow tree in the
/// background, which can be optionally be annotated with explanations or `reason`s as this crate
/// calls them
/// ```
///# use crate::*;
/// let arena = OpArena::new();
/// let (op, op_r) = Operation::make_ctors(&arena);
/// let one = op(1.0);
/// let two = op_r(2.0, "the number 2");
/// let one_plus_two = one + two;
/// let one_div_two =  one / two;
/// let prod = one_plus_two * one_div_two;
/// // by now, prod looks like the following
/// //        3
/// // 1 <-- (+) -> (2 "the number 2")
/// // ^      ^      ^
/// // |      |      |
/// // |      |      |
/// // |     (*)<--------- prod (1.5)
/// // |      |      |
/// // |      |      |
/// // |      |      |
/// // |      v      |
/// // |---- (/) ----|
/// //       0.5
/// ```
#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Operation<'a, Num> {
    op: OperationType<'a, Num>,
    reason: Option<Cow<'a, str>>,
    #[derivative(Debug = "ignore")]
    _allocator: &'a OpArena<'a, Num>,
}

impl<'a, Num> Operation<'a, Num> {
    /// Given an arena, which serves as the function context here, returns 2 closures, one that
    /// makes a reasonless Source, and one that makes a source with a reason. This is provided for
    /// convenience, so that the user doesn't need to pass the arena to a function each time they
    /// make a new operation.
    pub fn make_ctors(
        arena: &'a OpArena<'a, Num>,
    ) -> (
        impl Fn(Num) -> &'a Operation<'a, Num>,
        impl Fn(Num, &'static str) -> &'a Operation<'a, Num>,
    ) {
        (
            |i| Operation::new(i, arena),
            |i, reason| Operation::new_with_reason(i, reason, arena),
        )
    }

    pub fn value(&'a self) -> &Num {
        self.op.value()
    }

    pub fn new(i: Num, arena: &'a OpArena<'a, Num>) -> &'a Self {
        arena.alloc(Operation {
            op: OperationType::Source { value: i },
            reason: None,
            _allocator: arena,
        })
    }
    pub fn new_with_reason(i: Num, reason: &'a str, arena: &'a OpArena<'a, Num>) -> &'a Self {
        arena.alloc(Operation {
            op: OperationType::Source { value: i },
            reason: Some(reason.into()),
            _allocator: arena,
        })
    }

    // impl_arithmetic!(sub_internal, OperationType::Difference, -, OperationType::make_difference, Num);
    // impl_arithmetic!(div_internal, OperationType::Quotient, /, OperationType::make_quotient, Num);
    // impl_arithmetic!(mul_internal, OperationType::Product, *, OperationType::make_product, Num);
}

impl<'a, Num> Operation<'a, Num>
where
    Num: std::fmt::Display,
{
    /// outputs the operation and its history in dot format, which can be rendered with GraphViz
    pub fn as_graphviz(&'a self) -> String {
        let graph = visualization::OperationGraph::from_op(self);
        graph.to_graphviz()
    }
}

impl<'a, Num> Operation<'a, Num>
where
    Num: std::ops::Add + std::ops::Add<Output = Num>,
    Num: Clone,
{
    impl_arithmetic!(add_internal, OperationType::Sum, +, OperationType::make_sum, Num);
}

overload_operator!(std::ops::Add, Operation::add_internal, add);
overload_operator_commented!(
    std::ops::Add<(&'a Operation<'a, Num>, T)>,
    Operation::add_internal,
    add,
    T
);

// overload_operator!(std::ops::Sub, Operation::sub_internal, sub);
// overload_operator_commented!(
//     std::ops::Sub<(&'a Operation<'a, Num>, T)>,
//     Operation::sub_internal,
//     sub,
//     T
// );
//
// overload_operator!(std::ops::Mul, Operation::mul_internal, mul);
// overload_operator_commented!(
//     std::ops::Mul<(&'a Operation<'a, Num>, T)>,
//     Operation::mul_internal,
//     mul,
//     T
// );
//
// overload_operator!(std::ops::Div, Operation::div_internal, div);
// overload_operator_commented!(
//     std::ops::Div<(&'a Operation<'a, Num>, T)>,
//     Operation::div_internal,
//     div,
//     T
// );

/// Custom-defined functions which may take any number of arguments. For example, you might do
/// square root operations often, and decide to implement Operator for sqrt. This ends up being
/// dymanically dispatched in the graph however, so benchmark things and maybe modify the crate if
/// you think it's too slow
pub trait Operator<Num>: Debug {
    /// How should this operator be displayed
    fn symbol(&self) -> &'static str;
    /// What the operator does to targets. sqrt's might look something like
    /// ```
    /// let op = ops[0];
    /// Operation::new(f32::sqrt(op.value()), op._allocator)
    /// ```
    fn operate<'a>(&'a self, ops: &[&'a Operation<'a, Num>]) -> &'a Operation<'a, Num>;
}

#[derive(Debug, Clone)]
pub enum OperationType<'a, Num> {
    Source {
        value: Num,
    },
    Sum {
        value: Num,
        history: History<'a, Num>,
    },
    Difference {
        value: Num,
        history: History<'a, Num>,
    },
    Product {
        value: Num,
        history: History<'a, Num>,
    },
    Quotient {
        value: Num,
        history: History<'a, Num>,
    },
    Other {
        value: Num,
        op: &'a dyn Operator<Num>,
        history: History<'a, Num>,
    },
}

impl<'a, Num> OperationType<'a, Num> {
    fn variant_symbol(&self) -> &'static str {
        use OperationType::*;
        match self {
            Source { .. } => " ",
            Sum { .. } => " (+) ",
            Difference { .. } => " (-) ",
            Product { .. } => " (*) ",
            Quotient { .. } => " (/) ",
            Other { op, .. } => op.symbol(),
        }
    }

    fn history(&self) -> &[&'a Operation<'a, Num>] {
        use OperationType::*;
        match self {
            Source { .. } => panic!("don't ask for history on a leaf"),
            Sum { history, .. } => &history[..],
            Difference { history, .. } => &history[..],
            Product { history, .. } => &history[..],
            Quotient { history, .. } => &history[..],
            Other { history, .. } => &history[..],
        }
    }

    fn value(&self) -> &Num {
        use OperationType::*;
        match self {
            Source { value, .. } => value,
            Sum { value, .. } => value,
            Difference { value, .. } => value,
            Product { value, .. } => value,
            Quotient { value, .. } => value,
            Other { value, .. } => value,
        }
    }
    fn make_sum(value: Num, history: History<'a, Num>) -> OperationType<'a, Num> {
        OperationType::Sum { value, history }
    }
    fn make_difference(value: Num, history: History<'a, Num>) -> OperationType<'a, Num> {
        OperationType::Difference { value, history }
    }
    fn make_product(value: Num, history: History<'a, Num>) -> OperationType<'a, Num> {
        OperationType::Product { value, history }
    }
    fn make_quotient(value: Num, history: History<'a, Num>) -> OperationType<'a, Num> {
        OperationType::Quotient { value, history }
    }
}
