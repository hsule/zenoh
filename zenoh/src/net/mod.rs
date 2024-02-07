//
// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

//! ⚠️ WARNING ⚠️
//!
//! This module is intended for Zenoh's internal use.
//!
//! [Click here for Zenoh's documentation](../zenoh/index.html)
#[doc(hidden)]
// ignore_tagging
pub(crate) mod codec;
#[doc(hidden)]
// ignore_tagging
pub(crate) mod primitives;
#[doc(hidden)]
// ignore_tagging
pub(crate) mod protocol;
#[doc(hidden)]
// ignore_tagging
pub(crate) mod routing;
#[doc(hidden)]
// ignore_tagging
pub mod runtime;

#[cfg(test)]
// ignore_tagging
pub(crate) mod tests;
