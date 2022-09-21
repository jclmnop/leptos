#![feature(fn_traits)]
#![feature(let_chains)]
#![feature(unboxed_closures)]
#![feature(test)]

mod context;
mod effect;
mod hydration;
mod memo;
mod resource;
mod runtime;
mod scope;
mod signal;
mod source;
mod spawn;
mod subscriber;
mod suspense;

#[cfg(feature = "transition")]
mod transition;

pub use context::*;
pub use effect::*;
pub use memo::*;
pub use resource::*;
use runtime::*;
pub use scope::*;
pub use signal::*;
use source::*;
use spawn::*;
use subscriber::*;
pub use suspense::*;

#[cfg(feature = "transition")]
pub use transition::*;

#[macro_export]
macro_rules! debug_warn {
    ($($x:tt)*) => {
        {
            #[cfg(debug_assertions)]
            {
                log::warn!($($x)*)
            }
            #[cfg(not(debug_assertions))]
            { }
        }
    }
}

extern crate test;

#[cfg(test)]
mod tests {
    use test::Bencher;

    use std::{cell::Cell, rc::Rc};

    #[bench]
    fn create_and_update_1000_signals(b: &mut Bencher) {
        use crate::{create_effect, create_memo, create_scope, create_signal};

        b.iter(|| {
            create_scope(|cx| {
                let acc = Rc::new(Cell::new(0));
                let sigs = (0..1000).map(|n| create_signal(cx, n)).collect::<Vec<_>>();
                let reads = sigs.iter().map(|(r, _)| *r).collect::<Vec<_>>();
                let writes = sigs.iter().map(|(_, w)| *w).collect::<Vec<_>>();
                let memo = create_memo(cx, move |_| reads.iter().map(|r| r.get()).sum::<i32>());
                assert_eq!(memo(), 499500);
                create_effect(cx, {
                    let acc = Rc::clone(&acc);
                    move |_| {
                        acc.set(memo());
                    }
                });
                assert_eq!(acc.get(), 499500);

                writes[1].update(|n| *n += 1);
                writes[10].update(|n| *n += 1);
                writes[100].update(|n| *n += 1);

                assert_eq!(acc.get(), 499503);
                assert_eq!(memo(), 499503);
            })
            .dispose()
        });
    }

    #[bench]
    fn create_and_dispose_1000_scopes(b: &mut Bencher) {
        use crate::{create_effect, create_scope, create_signal};

        b.iter(|| {
            let acc = Rc::new(Cell::new(0));
            let disposers = (0..1000)
                .map(|_| {
                    create_scope({
                        let acc = Rc::clone(&acc);
                        move |cx| {
                            let (r, w) = create_signal(cx, 0);
                            create_effect(cx, {
                                move |_| {
                                    acc.set(r());
                                }
                            });
                            w(|n| *n += 1);
                        }
                    })
                })
                .collect::<Vec<_>>();
            for disposer in disposers {
                disposer.dispose();
            }
        });
    }

    #[bench]
    fn sycamore_create_and_update_1000_signals(b: &mut Bencher) {
        use sycamore::reactive::{create_effect, create_memo, create_scope, create_signal};

        b.iter(|| {
            let d = create_scope(|cx| {
                let acc = Rc::new(Cell::new(0));
                let sigs = Rc::new((0..1000).map(|n| create_signal(cx, n)).collect::<Vec<_>>());
                let memo = create_memo(cx, {
                    let sigs = Rc::clone(&sigs);
                    move || sigs.iter().map(|r| *r.get()).sum::<i32>()
                });
                assert_eq!(*memo.get(), 499500);
                create_effect(cx, {
                    let acc = Rc::clone(&acc);
                    move || {
                        acc.set(*memo.get());
                    }
                });
                assert_eq!(acc.get(), 499500);

                sigs[1].set(*sigs[1].get() + 1);
                sigs[10].set(*sigs[10].get() + 1);
                sigs[100].set(*sigs[100].get() + 1);

                assert_eq!(acc.get(), 499503);
                assert_eq!(*memo.get(), 499503);
            });
            unsafe { d.dispose() };
        });
    }

    #[bench]
    fn sycamore_create_and_dispose_1000_scopes(b: &mut Bencher) {
        use sycamore::reactive::{create_effect, create_scope, create_signal};

        b.iter(|| {
            let acc = Rc::new(Cell::new(0));
            let disposers = (0..1000)
                .map(|_| {
                    create_scope({
                        let acc = Rc::clone(&acc);
                        move |cx| {
                            let s = create_signal(cx, 0);
                            create_effect(cx, {
                                move || {
                                    acc.set(*s.get());
                                }
                            });
                            s.set(*s.get() + 1);
                        }
                    })
                })
                .collect::<Vec<_>>();
            for disposer in disposers {
                unsafe {
                    disposer.dispose();
                }
            }
        });
    }
}
