#[macro_export]
macro_rules! match_unordered {
    ($pattern1:pat, $pattern2:pat $(,)?) => {
        ($pattern1, $pattern2) | ($pattern2, $pattern1)
    };
}

#[macro_export]
macro_rules! impl_arithmetic {
    ($fname:tt, $OpVariant:path, $operator:tt, $variant_ctor:path, $Num:path) => {
        fn $fname(&'a self, other: &'a $crate::Operation<'a, $Num>) -> &'a mut Self {
            use $crate::OperationType::Source;
            let self_value = self.value();
            let other_value = other.value();
        match (self, other) {
            // $OpVariant $operator Source
            // happy path: we have a summed one and we fold 1 more into it, tack it on, keep the
            // sum's reason
            match_unordered!(
                foldee @ $crate::Operation {
                    op: Source { .. },
                    ..
                },
                $crate::Operation {
                    op: $OpVariant { history, .. },
                    reason,
                    ..
                },
            ) => self._allocator.alloc($crate::Operation {
                op: $variant_ctor ( self_value $operator other_value,
                                 Vec::from_iter(history.iter().copied().chain(once(foldee))),
                                 ),
                reason: reason.clone(),
                _allocator: self._allocator,
            }),
            // 2 sources (just numbers) put together, no reason given, not gonna derive one
            (
                $crate::Operation {
                    op: Source { value: a },
                    ..
                },
                $crate::Operation {
                    op: Source { value: b },
                    ..
                },
            ) => self._allocator.alloc($crate::Operation {
                op: $variant_ctor(a $operator b, vec![self, other]),
                reason: None,
                _allocator: self._allocator,
            }),
            // $OpVariant $operator $OpVariant, at least 1 with no reason. Fold them in and keep the chain short
            match_unordered!(
                $crate::Operation {
                    op: $OpVariant {
                        history: hist_a,
                        ..
                    },
                    reason,
                    ..
                },
                $crate::Operation {
                    op: $OpVariant {
                        history: hist_b,
                        ..
                    },
                    reason: None,
                    ..
                }
            ) => self._allocator.alloc($crate::Operation {
                op: $variant_ctor(self_value $operator other_value, hist_a
                                 .iter()
                                 .copied()
                                 .chain(hist_b.iter().copied())
                                 .collect()
                                 ),
                reason: reason.clone(),
                _allocator: self._allocator,
            }),
            // $OpVariant 2 things with reasons for each, make a new sum with no reason, listing both
            // sources in the "history" since we're combining semantically different sums and
            // want to preserve the history
            ($crate::Operation { op: a, .. }, $crate::Operation { op: b, .. }) => {
                self._allocator.alloc($crate::Operation {
                    op: $variant_ctor(a.value() $operator b.value(),
                    vec![self, other],
                    ),
                    reason: None,
                    _allocator: self._allocator,
                })
            }
        }
    }
    };
}

#[macro_export]
macro_rules! overload_operator {
    ($($pathpart:ident)::+, $func:path, $traitfunc:ident) => {
        impl<'a, Num> $($pathpart)::+ for &'a $crate::Operation<'a, Num>
        where
            Num: 'static,
            Num: $($pathpart)::+ + $($pathpart)::+<Output = Num>,
            &'a Num: $($pathpart)::+<&'a Num>,
            &'a Num: $($pathpart)::+ + $($pathpart)::+<Output = &'a Num>,
        {
            type Output = &'a $crate::Operation<'a, Num>;
            fn $traitfunc(self, _other: Self) -> Self::Output {
                todo!()
                // $func(self, other)
            }
        }
    };
}

#[macro_export]
macro_rules! overload_operator_commented {
    ($trait:path, $func:path, $traitfunc:ident, $typ:tt) => {
        impl<'a, $typ, Num> $trait for &'a $crate::Operation<'a, Num>
        where
            $typ: Into<Cow<'a, str>>,
        {
            type Output = &'a $crate::Operation<'a, Num>;
            fn $traitfunc(self, other: $crate::OpTuple<'a, Num, $typ>) -> Self::Output {
                let (other, reason) = other;
                let reason = Some(reason.into());
                let res = $func(self, other);
                res.reason = reason;
                res
            }
        }
    };
}
