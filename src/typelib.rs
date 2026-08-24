use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;

use itertools::Itertools;

use crate::error::Error;

pub(crate) struct TypeLibBuilder {
    next_type_id: TypeId,
    next_generic_type_id: GenericTypeDefId,
    type_lib: TypeLib,
    current_file: File,
}

impl TypeLibBuilder {
    fn new() -> Self {
        let mut new = Self {
            next_type_id: TypeId(0),
            next_generic_type_id: GenericTypeDefId(0),
            type_lib: TypeLib::new(),
            current_file: File {
                module: ModuleName::new(),
                usings: HashSet::new(),
            },
        };

        new.type_lib.module_by_full_name.insert(ModuleName::new(), Module::new(ModuleName::new()));

        macro_rules! add_default_types {
            ($($ty:ty),* $(,)?) => {
                $(_ = new.add_type(stringify!($ty), std::mem::size_of::<$ty>()));*
            };
        }

        add_default_types!(
            usize, isize,
            u8, i8,
            u16, i16,
            u32, i32,
            u64, i64,
            u128, i128,
            f32, f64,
        );

        new
    }

    pub fn add_all_modules_from_files<'a>(&mut self, files: impl Iterator<Item = &'a crate::ast::File>) -> Result<(), Error> {
        for file in files {
            let mut module = ModuleName::new();
            let mut parent = None;

            for name in file.module.get_names() {
                module.names.push(name);
                
                self.type_lib.module_by_full_name.entry(module.clone())
                    .or_insert_with(|| Module::new(module.clone()));

                if let Some(parent) = parent.replace(module.clone()) {
                    let Some(parent) = self.type_lib.module_by_full_name.get_mut(&parent) else {
                        return Err(Error::Unexpected("Failed to find parent of added module.".to_owned()));
                    };

                    parent.modules.insert(module.clone());
                }
            }
        }

        Ok(())
    }

    pub fn set_current_file(&mut self, file: &crate::ast::File) {
        let module = (&file.module).into();
        let usings = file.usings.iter()
            .map_into()
            .collect();

        self.current_file = File {
            module,
            usings,
        };
    }

    fn add<T, I: Id>(module_name: &ModuleName, modules: &mut HashMap<ModuleName, Module>, module_field: impl FnOnce(&mut Module) -> &mut HashSet<I>,
        next_id: &mut I, by_id: &mut HashMap<I, T>, kind: &str, new: impl FnOnce(I, ModuleName) -> T) -> Result<(), Error> {
        let id = next_id.next();

        let module = modules.get_mut(module_name)
            .ok_or_else(|| Error::Unexpected(format!("Tried to add {kind} to a module that hadn't been added yet.")))?;
        module_field(module)
            .insert(id)
            .then_some(())
            .ok_or_else(|| Error::Unexpected(format!("Tried to add {kind} to a module more than once.")))?;

        let new = new(id, module_name.clone());
        if by_id.insert(id, new).is_some() {
            return Err(Error::Unexpected(format!("Tried to insert a new {kind} with already existing type id.")));
        }

        Ok(())
    }

    pub fn add_type(&mut self, name: &str, size: usize) -> Result<(), Error> {
        Self::add(&self.current_file.module, &mut self.type_lib.module_by_full_name, Module::get_types_mut,
            &mut self.next_type_id, &mut self.type_lib.type_by_id, "type", |id, module| Type {
                id,
                size,
                extra: TypeSpecific::Named {
                    module,
                    name: name.to_owned(),
                    generic: None,
                },
            })
    }

    pub fn add_generic_type_def<T>(&mut self, name: &str, params: impl Iterator<Item = String>, fields: impl Iterator<Item = crate::ast::Type>) -> Result<(), Error> {
        Self::add(&self.current_file.module, &mut self.type_lib.module_by_full_name, Module::get_generic_types_mut,
            &mut self.next_generic_type_id, &mut self.type_lib.generic_type_by_id, "generic type def", |id, module| GenericTypeDef {
                id,
                module,
                name: name.to_owned(),
                params: params.collect(),
                fields: fields.collect(),
            })
    }

    fn is_full_module(&self, module_name: &ModuleName) -> bool {
        let len = module_name.names.len();

        for slice in (1..=len).map(|i| &module_name.names[..i]) {
            if self.type_lib.module_by_full_name.keys().any(|module| module == slice) {
                return true;
            }
        }

        false
    }

    fn is_empty_module(&self, module_name: &ModuleName) -> bool {
        module_name.names.is_empty()
    }

    fn find_module_in_usings(&self, module_name: &ModuleName) -> Result<impl Iterator<Item = ModuleName>, Error> {
        let Some((first_module_name, module_names)) = module_name.names.split_first() else {
            return Err(Error::Unexpected("Found module part without name.".to_owned()));
        };

        Ok(self.current_file.usings.iter().filter_map(|using| {
            let Some((last_using_name, using_names)) = using.names.split_last() else {
                return Some(Err(Error::Unexpected("Found using without name.".to_owned())));
            };

            if last_using_name != first_module_name {
                return None;
            }

            let combined_name = using_names.into_iter()
                .chain(module_names)
                .cloned()
                .collect_vec()
                .into();
            
            self.type_lib.module_by_full_name.contains_key(&combined_name)
                .then_some(Ok(combined_name))
        }).collect::<Result<Vec<_>, _>>()?.into_iter())
    }

    fn resolve_module(&self, module_part: &ModuleName) -> Result<impl Iterator<Item = ModuleName> + use<>, Error> {
        let mut possible = Vec::new();
        
        if self.is_empty_module(module_part) {
            possible.push(module_part.clone());
            possible.push(self.current_file.module.clone());
        }
        else {
            if self.is_full_module(module_part) {
                possible.push(module_part.clone());
            }

            possible.extend(self.find_module_in_usings(module_part)?);

            if possible.len() > 1 {
                return Err(Error::AmbiguousModule { name: module_part.clone(), possible });
            }
        }
        
        Ok(possible.into_iter())
    }

    pub fn resolve_type(&mut self, r#type: &crate::ast::Type) -> Result<TypeId, Error> {
        use crate::ast::Type;

        match r#type {
            Type::Array(array) => self.resolve_array_type(array),
            Type::Pointer(pointer) => self.resolve_pointer_type(pointer),
            Type::Function(function) => self.resolve_function_type(function),
            Type::Named(named) => self.resolve_named_type(named),
        }
    }

    fn resolve_array_type(&mut self, array_type: &crate::ast::ArrayType) -> Result<TypeId, Error> {
        let instance = ArrayInstance {
            count: array_type.count.value,
            item: self.resolve_type(&array_type.item_type)?,
        };

        let item_size = self.type_lib.get_type(instance.item).size;

        let instance_id = *self.type_lib.type_by_array_instance.entry(instance)
            .or_insert_with(|| {
                let new_id = self.next_type_id.next();

                let new_array_type = Type {
                    id: new_id,
                    size: item_size * instance.count,
                    extra: TypeSpecific::Array(instance),
                };

                self.type_lib.type_by_id.insert(new_array_type.id, new_array_type);

                new_id
            });

        Ok(instance_id)
    }

    fn resolve_pointer_type(&mut self, pointer_type: &crate::ast::PointerType) -> Result<TypeId, Error> {
        let instance = PointerInstance {
            inner: self.resolve_type(&pointer_type.inner_type)?,
        };

        let instance_id = *self.type_lib.type_by_pointer_instance.entry(instance)
            .or_insert_with(|| {
                let new_id = self.next_type_id.next();

                let new_pointer_type = Type {
                    id: new_id,
                    size: size_of::<usize>(),
                    extra: TypeSpecific::Pointer(instance),
                };

                self.type_lib.type_by_id.insert(new_pointer_type.id, new_pointer_type);

                new_id
            });

        Ok(instance_id)
    }

    fn resolve_function_type(&mut self, function_type: &crate::ast::FunctionType) -> Result<TypeId, Error> {
        let call_conv = self.resolve_call_conv(function_type.call_conv.as_ref())?;

        let generics = function_type.generics.as_ref()
            .map(|g|
                g.names.iter().map(|w| &w.value)
            ).into_iter()
            .flatten()
            .collect_vec();

        let mut arg_to_arg = |arg: &crate::ast::Type| {
            if let crate::ast::Type::Named(crate::ast::NamedType { name, generics: None, .. }) = arg
                && let [crate::ast::WithSpan { value: name, .. }] = name.names.as_slice()
                && let Some(pos) = generics.iter().position(|&x| x == name) {
                Ok(FunctionArg::GenericType(pos))
            } else {
                Ok(FunctionArg::ConcreteType(self.resolve_type(&arg)?))
            }
        };
        let args = function_type.args.iter()
                .map(|arg| arg_to_arg(&arg.r#type))
                .collect::<Result<_, _>>()?;
        let ret = function_type.ret.as_ref()
                .map(|ret| arg_to_arg(&ret))
                .transpose()?;

        let instance = FunctionInstance { call_conv, args, ret };
        
        let instance_id = *self.type_lib.type_by_function_instance.entry(instance.clone())
            .or_insert_with(|| {
                let new_id = self.next_type_id.next();

                let new_function_type = Type {
                    id: new_id,
                    size: size_of::<*const ()>(),
                    extra: TypeSpecific::Function(instance)
                };

                self.type_lib.type_by_id.insert(new_function_type.id, new_function_type);

                new_id
            });
        
        Ok(instance_id)
    }

    fn resolve_call_conv(&self, call_conv: Option<&crate::ast::WithSpan<String>>) -> Result<CallConv, Error> {
        if let Some(call_conv) = call_conv {
            let call_conv = &call_conv.value;

            Ok(match call_conv.as_str() {
                "cdecl" => CallConv::CDecl,
                "fastcall" => CallConv::FastCall,
                "stdcall" => CallConv::StdCall,
                "thiscall" => CallConv::ThisCall,
                "vectorcall" => CallConv::VectorCall,
                unk => Err(Error::UnknownCallConv { name: unk.to_owned() })?,
            })
        } else {
            // TODO: have the typelib know what the target is
            Ok(CallConv::CDecl)
        }
    }

    fn resolve_named_type(&mut self, named_type: &crate::ast::NamedType) -> Result<TypeId, Error> {
        let (possible_modules, name) = match named_type.name.names.as_slice() {
            [module @ .., name] => {
                let module_part = module.iter().map(|w| w.value.clone()).collect_vec().into();
                let possible = self.resolve_module(&module_part)?;

                (possible, name.value.clone())
            }
            [] => {
                return Err(Error::Unexpected("Found named type with no name.".to_owned()));
            }
        };

        if let Some(generics) = &named_type.generics {
            let mut possible = Vec::new();

            for module in possible_modules {
                let module = &self.type_lib.module_by_full_name[&module];

                for generic_type in &module.generic_types {
                    let generic_type = &self.type_lib.generic_type_by_id[generic_type];

                    if generic_type.name == name && generic_type.params.len() == generics.types.len() {
                        possible.push((module.name.clone(), generic_type.id));
                    }
                }
            }
            
            let Some(((module_name, generic_def), others)) = possible.split_first() else {
                return Err(Error::UnknownType { r#type: named_type.clone() });
            };

            if !others.is_empty() {
                return Err(Error::AmbiguousType {
                    name: named_type.clone(),
                    possible: possible.into_iter()
                        .map(|(m, _)| m)
                        .collect_vec(),
                });
            }

            let instance = GenericInstance {
                def: *generic_def,
                args: generics.types.iter()
                    .map(|t| self.resolve_type(t))
                    .collect::<Result<_, _>>()?,
            };
            
            let instance_id = *self.type_lib.type_by_generic_instance.entry(instance.clone())
                .or_insert_with(|| {
                    let new_id = self.next_type_id.next();

                    let new_type = Type {
                        id: new_id,
                        size: todo!("Get size for generic types"),
                        extra: TypeSpecific::Named {
                            module: module_name.clone(),
                            name,
                            generic: Some(instance),
                        },
                    };

                    self.type_lib.type_by_id.insert(new_type.id, new_type);

                    new_id
                });

            Ok(instance_id)
        } else {
            let mut possible = Vec::new();

            for module in possible_modules {
                let module = &self.type_lib.module_by_full_name[&module];

                for r#type in &module.types {
                    let r#type = &self.type_lib.type_by_id[&r#type];

                    if let TypeSpecific::Named { name: type_name, .. } = &r#type.extra && *type_name == name {
                        possible.push((module.name.clone(), r#type.id));
                    }
                }
            }
            
            let Some(((_, r#type), others)) = possible.split_first() else {
                return Err(Error::UnknownType { r#type: named_type.clone() });
            };

            if !others.is_empty() {
                return Err(Error::AmbiguousType {
                    name: named_type.clone(),
                    possible: possible.into_iter()
                        .map(|(m, _)| m)
                        .collect_vec(),
                });
            }

            Ok(*r#type)
        }
    }

    pub fn into_typelib(self) -> TypeLib {
        self.type_lib
    }
}

pub struct TypeLib {
    type_by_id: HashMap<TypeId, Type>,
    generic_type_by_id: HashMap<GenericTypeDefId, GenericTypeDef>,

    type_by_generic_instance: HashMap<GenericInstance, TypeId>,
    type_by_array_instance: HashMap<ArrayInstance, TypeId>,
    type_by_pointer_instance: HashMap<PointerInstance, TypeId>,
    type_by_function_instance: HashMap<FunctionInstance, TypeId>,

    module_by_full_name: HashMap<ModuleName, Module>,
}

impl TypeLib {
    fn new() -> Self {
        Self {
            type_by_id: HashMap::new(),
            generic_type_by_id: HashMap::new(),
            type_by_generic_instance: HashMap::new(),
            type_by_array_instance: HashMap::new(),
            type_by_pointer_instance: HashMap::new(),
            type_by_function_instance: HashMap::new(),
            module_by_full_name: HashMap::new(),
        }
    }

    pub fn get_type(&self, id: TypeId) -> &Type {
        &self.type_by_id[&id]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ModuleName {
    names: Vec<String>,
}

impl ModuleName {
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
        }
    }
}

impl<'a> From<&'a crate::ast::Module> for ModuleName {
    fn from(value: &'a crate::ast::Module) -> Self {
        value.get_names().into()
    }
}

impl From<Vec<String>> for ModuleName {
    fn from(value: Vec<String>) -> Self {
        Self { names: value }
    }
}

impl PartialEq<[String]> for ModuleName {
    fn eq(&self, other: &[String]) -> bool {
        self.names == other
    }
}

impl Display for ModuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.names.iter().format("::").to_string())
    }
}

struct Module {
    pub name: ModuleName,
    pub modules: HashSet<ModuleName>,
    pub types: HashSet<TypeId>,
    pub generic_types: HashSet<GenericTypeDefId>,
}

impl Module {
    pub fn new(name: ModuleName) -> Self {
        Module {
            name,
            modules: HashSet::new(),
            types: HashSet::new(),
            generic_types: HashSet::new(),
        }
    }

    pub fn get_types_mut(&mut self) -> &mut HashSet<TypeId> {
        &mut self.types
    }

    pub fn get_generic_types_mut(&mut self) -> &mut HashSet<GenericTypeDefId> {
        &mut self.generic_types
    }
}

trait Id: Copy + PartialEq + Eq + Hash {
    fn next(&mut self) -> Self;
}

macro_rules! DefineId {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(usize);

        impl Id for $name {
            fn next(&mut self) -> Self {
                let id = *self;
                self.0 += 1;
                id
            }
        }
    };
}

enum TypeSpecific {
    Array(ArrayInstance),
    Pointer(PointerInstance),
    Function(FunctionInstance),
    Named {
        module: ModuleName,
        name: String,
        generic: Option<GenericInstance>,
    },
}

DefineId!(TypeId);

struct Type {
    pub id: TypeId,
    pub size: usize,

    pub extra: TypeSpecific,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ArrayInstance {
    count: usize,
    item: TypeId,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PointerInstance {
    inner: TypeId,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FunctionInstance {
    call_conv: CallConv,
    args: Vec<FunctionArg>,
    ret: Option<FunctionArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CallConv {
    CDecl,
    FastCall,
    StdCall,
    ThisCall,
    VectorCall,
    Win64,
    SysV64,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum FunctionArg {
    GenericType(usize),
    ConcreteType(TypeId),
}

DefineId!(GenericTypeDefId);

struct GenericTypeDef {
    pub id: GenericTypeDefId,
    pub module: ModuleName,
    pub name: String,
    pub params: Vec<String>,
    pub fields: Vec<crate::ast::Type>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct GenericInstance {
    pub def: GenericTypeDefId,
    pub args: Vec<TypeId>,
}

struct File {
    pub module: ModuleName,
    pub usings: HashSet<ModuleName>,
}
