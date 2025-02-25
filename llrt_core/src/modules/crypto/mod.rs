// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
mod crc32;
mod md5_hash;
mod sha_hash;
use std::slice;

use once_cell::sync::Lazy;
use rand::{prelude::ThreadRng, Rng};
use ring::rand::{SecureRandom, SystemRandom};
use rquickjs::{
	function::{Constructor, Opt},
	module::{Declarations, Exports, ModuleDef},
	prelude::{Func, Rest},
	Class,
	Ctx,
	Error,
	Exception,
	Function,
	IntoJs,
	Null,
	Object,
	Result,
	Value,
};

use self::{
	crc32::{Crc32, Crc32c},
	md5_hash::Md5,
	sha_hash::{Hash, Hmac, ShaAlgorithm, ShaHash},
};
use crate::{
	module_builder::ModuleInfo,
	modules::{
		buffer::Buffer,
		encoding::encoder::{bytes_to_b64_string, bytes_to_hex_string},
		llrt::uuid::uuidv4,
		module::export_default,
	},
	utils::{
		class::get_class_name,
		object::{bytes_to_typed_array, get_start_end_indexes, obj_to_array_buffer},
		result::ResultExt,
	},
	vm::{CtxExtension, ErrorExtensions},
};

pub static SYSTEM_RANDOM:Lazy<SystemRandom> = Lazy::new(SystemRandom::new);

fn encoded_bytes<'js>(ctx:Ctx<'js>, bytes:&[u8], encoding:&str) -> Result<Value<'js>> {
	match encoding {
		"hex" => {
			let hex = bytes_to_hex_string(bytes);

			let hex = rquickjs::String::from_str(ctx, &hex)?;

			Ok(Value::from_string(hex))
		},
		"base64" => {
			let b64 = bytes_to_b64_string(bytes);

			let b64 = rquickjs::String::from_str(ctx, &b64)?;

			Ok(Value::from_string(b64))
		},
		_ => bytes_to_typed_array(ctx, bytes),
	}
}

#[inline]
pub fn random_byte_array(length:usize) -> Vec<u8> {
	let mut vec = vec![0; length];

	SYSTEM_RANDOM.fill(&mut vec).unwrap();

	vec
}

fn get_random_bytes(ctx:Ctx, length:usize) -> Result<Value> {
	let random_bytes = random_byte_array(length);

	Buffer(random_bytes).into_js(&ctx)
}

fn get_random_int(_ctx:Ctx, first:i64, second:Opt<i64>) -> Result<i64> {
	let mut rng = ThreadRng::default();

	let random_number = match second.0 {
		Some(max) => rng.gen_range(first..max),
		None => rng.gen_range(0..first),
	};

	Ok(random_number)
}

fn random_fill<'js>(ctx:Ctx<'js>, obj:Object<'js>, args:Rest<Value<'js>>) -> Result<()> {
	let args_iter = args.0.into_iter();

	let mut args_iter = args_iter.rev();

	let callback:Function = args_iter
		.next()
		.and_then(|v| v.into_function())
		.or_throw_msg(&ctx, "Callback required")?;

	let size = args_iter.next().and_then(|arg| arg.as_int()).map(|i| i as usize);

	let offset = args_iter.next().and_then(|arg| arg.as_int()).map(|i| i as usize);

	ctx.clone().spawn_exit(async move {
		if let Err(err) = random_fill_sync(ctx.clone(), obj.clone(), Opt(offset), Opt(size)) {
			let err = err.into_value(&ctx)?;
			() = callback.call((err,))?;

			return Ok(());
		}
		() = callback.call((Null.into_js(&ctx), obj))?;

		Ok::<_, Error>(())
	})?;

	Ok(())
}

fn random_fill_sync<'js>(
	ctx:Ctx<'js>,
	obj:Object<'js>,
	offset:Opt<usize>,
	size:Opt<usize>,
) -> Result<Object<'js>> {
	let offset = offset.unwrap_or(0);

	if let Some((array_buffer, source_length, source_offset)) = obj_to_array_buffer(&obj)? {
		let (start, end) = get_start_end_indexes(source_length, size.0, offset);

		let raw = array_buffer.as_raw().ok_or("ArrayBuffer is detached").or_throw(&ctx)?;

		let bytes = unsafe { slice::from_raw_parts_mut(raw.ptr.as_ptr(), source_length) };

		SYSTEM_RANDOM
			.fill(&mut bytes[start + source_offset..end - source_offset])
			.unwrap();
	}

	Ok(obj)
}

fn get_random_values<'js>(ctx:Ctx<'js>, obj:Object<'js>) -> Result<Object<'js>> {
	if let Some((array_buffer, source_length, source_offset)) = obj_to_array_buffer(&obj)? {
		let raw = array_buffer.as_raw().ok_or("ArrayBuffer is detached").or_throw(&ctx)?;

		if source_length > 0x10000 {
			return Err(Exception::throw_message(
				&ctx,
				"QuotaExceededError: The requested length exceeds 65,536 bytes",
			));
		}

		let bytes = unsafe {
			std::slice::from_raw_parts_mut(raw.ptr.as_ptr().add(source_offset), source_length)
		};

		match get_class_name(&obj)?.unwrap().as_str() {
			"Int8Array" | "Uint8Array" | "Uint8ClampedArray" | "Int16Array" | "Uint16Array"
			| "Int32Array" | "Uint32Array" | "BigInt64Array" | "BigUint64Array" => {
				SYSTEM_RANDOM.fill(bytes).unwrap()
			},
			_ => {
				return Err(Exception::throw_message(&ctx, "Unsupported TypedArray"));
			},
		}
	}

	Ok(obj)
}

pub fn init(ctx:&Ctx<'_>) -> Result<()> {
	let globals = ctx.globals();

	let crypto = Object::new(ctx.clone())?;

	crypto.set("createHash", Func::from(Hash::new))?;

	crypto.set("createHmac", Func::from(Hmac::new))?;

	crypto.set("randomBytes", Func::from(get_random_bytes))?;

	crypto.set("randomInt", Func::from(get_random_int))?;

	crypto.set("randomUUID", Func::from(uuidv4))?;

	crypto.set("randomFillSync", Func::from(random_fill_sync))?;

	crypto.set("randomFill", Func::from(random_fill))?;

	crypto.set("getRandomValues", Func::from(get_random_values))?;

	globals.set("crypto", crypto)?;

	Ok(())
}

pub struct CryptoModule;

impl ModuleDef for CryptoModule {
	fn declare(declare:&Declarations) -> Result<()> {
		declare.declare("createHash")?;

		declare.declare("createHmac")?;

		declare.declare("Crc32")?;

		declare.declare("Crc32c")?;

		declare.declare("Md5")?;

		declare.declare("randomBytes")?;

		declare.declare("randomUUID")?;

		declare.declare("randomInt")?;

		declare.declare("randomFillSync")?;

		declare.declare("randomFill")?;

		declare.declare("getRandomValues")?;

		for sha_algorithm in ShaAlgorithm::iterate() {
			let class_name = sha_algorithm.class_name();

			declare.declare(class_name)?;
		}

		declare.declare("default")?;

		Ok(())
	}

	fn evaluate<'js>(ctx:&Ctx<'js>, exports:&Exports<'js>) -> Result<()> {
		Class::<Hash>::register(ctx)?;

		Class::<Hmac>::register(ctx)?;

		Class::<ShaHash>::register(ctx)?;

		export_default(ctx, exports, |default| {
			for sha_algorithm in ShaAlgorithm::iterate() {
				let class_name:&str = sha_algorithm.class_name();

				let algo = sha_algorithm;

				let ctor =
					Constructor::new_class::<ShaHash, _, _>(ctx.clone(), move |ctx, secret| {
						ShaHash::new(ctx, algo, secret)
					})?;

				default.set(class_name, ctor)?;
			}

			Class::<Md5>::define(default)?;

			Class::<Crc32>::define(default)?;

			Class::<Crc32c>::define(default)?;

			default.set("createHash", Func::from(Hash::new))?;

			default.set("createHmac", Func::from(Hmac::new))?;

			default.set("randomBytes", Func::from(get_random_bytes))?;

			default.set("randomInt", Func::from(get_random_int))?;

			default.set("randomUUID", Func::from(uuidv4))?;

			default.set("randomFillSync", Func::from(random_fill_sync))?;

			default.set("randomFill", Func::from(random_fill))?;

			default.set("getRandomValues", Func::from(get_random_values))?;

			Ok(())
		})?;

		Ok(())
	}
}

impl From<CryptoModule> for ModuleInfo<CryptoModule> {
	fn from(val:CryptoModule) -> Self { ModuleInfo { name:"crypto", module:val } }
}
