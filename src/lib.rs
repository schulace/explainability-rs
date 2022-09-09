use derivative::Derivative;
use serde::Serialize;
use std::{borrow::Cow, fmt::Debug, iter::once};
type Num = f32;

mod macros;
#[cfg(test)]
mod testing;
mod visualization;

pub(crate) type OpTuple<'a, R> = (&'a Operation<'a>, R);
type History<'a> = Vec<&'a Operation<'a>>;
type OpArena<'a> = typed_arena::Arena<Operation<'a>>;

#[derive(Derivative, Serialize, Clone)]
#[derivative(Debug)]
pub struct Operation<'a> {
    op: OperationType<'a>,
    reason: Option<Cow<'a, str>>,
    #[serde(skip)]
    #[derivative(Debug = "ignore")]
    _allocator: &'a OpArena<'a>,
}

impl<'a> Operation<'a> {
    pub fn new(i: Num, arena: &'a OpArena<'a>) -> &'a Self {
        arena.alloc(Operation {
            op: OperationType::Source { value: i },
            reason: None,
            _allocator: arena,
        })
    }
    pub fn new_with_reason(i: Num, reason: &'a str, arena: &'a OpArena<'a>) -> &'a Self {
        arena.alloc(Operation {
            op: OperationType::Source { value: i },
            reason: Some(reason.into()),
            _allocator: arena,
        })
    }

    pub fn as_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn as_graphviz(&'a self) -> String {
        let graph = visualization::OperationGraph::from_op(self);
        graph.to_graphviz()
    }

    impl_arithmetic!(add_internal, OperationType::Sum, +, OperationType::make_sum);
    impl_arithmetic!(sub_internal, OperationType::Difference, -, OperationType::make_difference);
    impl_arithmetic!(div_internal, OperationType::Quotient, /, OperationType::make_quotient);
    impl_arithmetic!(mul_internal, OperationType::Product, *, OperationType::make_product);
}

overload_operator!(std::ops::Add, Operation::add_internal, add);
overload_operator_commented!(
    std::ops::Add<(&'a Operation<'a>, T)>,
    Operation::add_internal,
    add,
    T
);

overload_operator!(std::ops::Sub, Operation::sub_internal, sub);
overload_operator_commented!(
    std::ops::Sub<(&'a Operation<'a>, T)>,
    Operation::sub_internal,
    sub,
    T
);

overload_operator!(std::ops::Mul, Operation::mul_internal, mul);
overload_operator_commented!(
    std::ops::Mul<(&'a Operation<'a>, T)>,
    Operation::mul_internal,
    mul,
    T
);

overload_operator!(std::ops::Div, Operation::div_internal, div);
overload_operator_commented!(
    std::ops::Div<(&'a Operation<'a>, T)>,
    Operation::div_internal,
    div,
    T
);

pub trait Operator: Debug {
    fn symbol(&self) -> &'static str;
    fn operate<'a>(&'a self, ops: &[&'a Operation<'a>]) -> &'a Operation;
}

#[derive(Serialize, Debug, Clone)]
pub enum OperationType<'a> {
    Source {
        value: Num,
    },
    Sum {
        value: Num,
        history: History<'a>,
    },
    Difference {
        value: Num,
        history: History<'a>,
    },
    Product {
        value: Num,
        history: History<'a>,
    },
    Quotient {
        value: Num,
        history: History<'a>,
    },
    Other {
        value: Num,
        #[serde(skip)]
        op: &'a dyn Operator,
        history: History<'a>,
    },
}

impl<'a> OperationType<'a> {
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

    fn history(&self) -> &[&'a Operation<'a>] {
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

    fn value(&self) -> Num {
        use OperationType::*;
        match self {
            Source { value, .. } => *value,
            Sum { value, .. } => *value,
            Difference { value, .. } => *value,
            Product { value, .. } => *value,
            Quotient { value, .. } => *value,
            Other { value, .. } => *value,
        }
    }
    fn make_sum(value: Num, history: History<'a>) -> OperationType<'a> {
        OperationType::Sum { value, history }
    }
    fn make_difference(value: Num, history: History<'a>) -> OperationType<'a> {
        OperationType::Difference { value, history }
    }
    fn make_product(value: Num, history: History<'a>) -> OperationType<'a> {
        OperationType::Product { value, history }
    }
    fn make_quotient(value: Num, history: History<'a>) -> OperationType<'a> {
        OperationType::Quotient { value, history }
    }
}
