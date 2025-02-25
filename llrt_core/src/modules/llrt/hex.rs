// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
pub mod encoder {
	pub use llrt_utils::encoding::*;
}

use rquickjs::{
	module::{Declarations, Exports, ModuleDef},
	prelude::Func,
	Ctx,
	Result,
	Value,
};

use self::encoder::{bytes_from_hex, bytes_to_hex_string};
use crate::{
	module_builder::ModuleInfo,
	modules::module::export_default,
	utils::{
		object::{bytes_to_typed_array, get_bytes},
		result::ResultExt,
	},
};

pub struct LlrtHexModule;

impl LlrtHexModule {
	pub fn encode<'js>(ctx:Ctx<'js>, buffer:Value<'js>) -> Result<String> {
		let bytes = get_bytes(&ctx, buffer)?;

		Ok(bytes_to_hex_string(&bytes))
	}

	pub fn decode(ctx:Ctx, encoded:String) -> Result<Value> {
		let bytes = bytes_from_hex(encoded.as_bytes())
			.or_throw_msg(&ctx, "Cannot decode unrecognized sequence")?;

		bytes_to_typed_array(ctx, &bytes)
	}
}

impl ModuleDef for LlrtHexModule {
	fn declare(declare:&Declarations) -> Result<()> {
		declare.declare(stringify!(encode))?;

		declare.declare(stringify!(decode))?;

		declare.declare("default")?;

		Ok(())
	}

	fn evaluate<'js>(ctx:&Ctx<'js>, exports:&Exports<'js>) -> Result<()> {
		export_default(ctx, exports, |default| {
			default.set(stringify!(encode), Func::from(Self::encode))?;

			default.set(stringify!(decode), Func::from(Self::decode))?;

			Ok(())
		})?;

		Ok(())
	}
}

impl From<LlrtHexModule> for ModuleInfo<LlrtHexModule> {
	fn from(val:LlrtHexModule) -> Self { ModuleInfo { name:"llrt:hex", module:val } }
}
