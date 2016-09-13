// Copyright (c) IxMilia.  All Rights Reserved.  Licensed under the Apache License, Version 2.0.  See License.txt in the project root for license information.

extern crate xmltree;
use self::xmltree::Element;

use ::{ExpectedType, get_code_pair_type, get_expected_type};

use xml_helpers::*;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Write};
use std::iter::Iterator;

pub fn generate_entities() {
    let element = load_xml();
    let mut fun = String::new();
    fun.push_str("
// The contents of this file are automatically generated and should not be modified directly.  See the `build` directory.

use ::{CodePair, CodePairAsciiWriter, Color, LwPolylineVertex, Point, Vector};
use ::helper_functions::*;

use enums::*;
use enum_primitive::FromPrimitive;

use std::io;
use std::io::Write;

// Used to turn Option<T> into io::Result.
macro_rules! try_result {
    ($expr : expr) => (
        match $expr {
            Some(v) => v,
            None => return Err(io::Error::new(io::ErrorKind::InvalidData, \"unexpected enum value\"))
        }
    )
}
".trim_left());
    fun.push_str("\n");
    generate_base_entity(&mut fun, &element);
    generate_entity_types(&mut fun, &element);

    fun.push_str("impl EntityType {\n");
    generate_is_supported_on_version(&mut fun, &element);
    generate_type_string(&mut fun, &element);
    generate_try_apply_code_pair(&mut fun, &element);
    generate_write(&mut fun, &element);
    fun.push_str("}\n");

    let mut file = File::create("src/generated/entities.rs").ok().unwrap();
    file.write_all(fun.as_bytes()).ok().unwrap();
}

fn generate_base_entity(fun: &mut String, element: &Element) {
    let entity = &element.children[0];
    if name(&entity) != "Entity" { panic!("Expected first entity to be 'Entity'."); }
    fun.push_str("#[derive(Clone)]\n");
    fun.push_str("pub struct EntityCommon {\n");
    for c in &entity.children {
        let t = if allow_multiples(&c) { format!("Vec<{}>", typ(c)) } else { typ(c) };
        match &*c.name {
            "Field" => {
                fun.push_str(&format!("    pub {name}: {typ},\n", name=name(c), typ=t));
            },
            "Pointer" => {
                // TODO: proper handling of pointers
                let typ = if allow_multiples(&c) { "Vec<u32>" } else { "u32" };
                fun.push_str(&format!("    pub {name}: {typ},\n", name=name(c), typ=typ));
            },
            "WriteOrder" => (),
            _ => panic!("unexpected element under Entity: {}", c.name),
        }
    }

    fun.push_str("}\n");
    fun.push_str("\n");

// for large sections like this, you might look into using multiline strings
// they look like this
// fun.push_str(#"""
//   foo bar baz
//   foo bar baz
//   foo bar baz
// """#)
//#
    fun.push_str("#[derive(Clone)]\n");
    fun.push_str("pub struct Entity {\n");
    fun.push_str("    pub common: EntityCommon,\n");
    fun.push_str("    pub specific: EntityType,\n");
    fun.push_str("}\n");
    fun.push_str("\n");

    fun.push_str("impl Default for EntityCommon {\n");
    fun.push_str("    fn default() -> EntityCommon {\n");
    fun.push_str("        EntityCommon {\n");
    for c in &entity.children {
        match &*c.name {
            "Field" | "Pointer" => {
                // TODO: proper handling of pointers
                let default_value = if c.name == "Field" { default_value(&c) } else { String::from("0") };
                fun.push_str(&format!("            {name}: {val},\n", name=name(c), val=default_value));
            },
            "WriteOrder" => (),
            _ => panic!("unexpected element under Entity: {}", c.name),
        }
    }

    fun.push_str("        }\n");
    fun.push_str("    }\n");
    fun.push_str("}\n");
    fun.push_str("\n");

    fun.push_str("impl EntityCommon {\n");
    fun.push_str("    pub fn new() -> Self {\n");
    fun.push_str("        Default::default()\n");
    fun.push_str("    }\n");

    ////////////////////////////////////////////////////// apply_individual_pair
    fun.push_str("    pub fn apply_individual_pair(&mut self, pair: &CodePair) -> io::Result<()> {\n");
    fun.push_str("        match pair.code {\n");
    for c in &entity.children {
        if c.name == "Field" {
            let read_fun = if allow_multiples(&c) {
                format!(".push({})", get_field_reader(&c))
            }
            else {
                format!(" = {}", get_field_reader(&c))
            };
            fun.push_str(&format!("            {code} => {{ self.{field}{read_fun} }},\n", code=code(c), field=name(c), read_fun=read_fun));
        }
        else if c.name == "Pointer" {
            // TODO: proper handling of pointers
            fun.push_str(&format!("            {code} => {{ self.{field} = try!(as_u32(string_value(&pair.value))) }},\n", code=code(&c), field=name(c)));
        }
    }

    fun.push_str("            _ => (), // unknown code, just ignore\n");
    fun.push_str("        }\n");
    fun.push_str("        Ok(())\n");
    fun.push_str("    }\n");

    ////////////////////////////////////////////////////////////////////// write
    fun.push_str("    pub fn write<T>(&self, version: &AcadVersion, write_handles: bool, writer: &mut CodePairAsciiWriter<T>) -> io::Result<()>\n");
    fun.push_str("        where T: Write {\n");
    fun.push_str("        let ent = self;\n");
    for line in generate_write_code_pairs(&entity) {
        fun.push_str(&format!("        {}\n", line));
    }

    fun.push_str("        Ok(())\n");
    fun.push_str("    }\n");

    fun.push_str("}\n");
    fun.push_str("\n");
}

fn generate_entity_types(fun: &mut String, element: &Element) {
    fun.push_str("#[derive(Clone)]\n");
    fun.push_str("pub enum EntityType {\n");
    for c in &element.children {
        if c.name != "Entity" { panic!("expected top level entity"); }
        if name(c) != "Entity" && name(c) != "DimensionBase" && attr(&c, "BaseClass") != "DimensionBase" {
            // TODO: handle dimensions
            // TODO: handle complex subtypes: e.g., lwpolyline has vertices
            fun.push_str(&format!("    {typ}({typ}),\n", typ=name(c)));
        }
    }

    fun.push_str("}\n");
    fun.push_str("\n");

    // individual structs
    for c in &element.children {
        if c.name != "Entity" { panic!("expected top level entity"); }
        if name(c) != "Entity" && name(c) != "DimensionBase" && attr(&c, "BaseClass") != "DimensionBase" {
            // TODO: handle dimensions
            // TODO: handle complex subtypes: e.g., lwpolyline has vertices

            // definition
            fun.push_str("#[derive(Clone, Debug, PartialEq)]\n");
            fun.push_str(&format!("pub struct {typ} {{\n", typ=name(c)));
            for f in &c.children {
                let t = if allow_multiples(&f) { format!("Vec<{}>", typ(f)) } else { typ(f) };
                let acc = if attr(&f, "Accessibility") == "private" { "" } else { "pub " };
                match &*f.name {
                    "Field" => {
                        fun.push_str(&format!("    {acc}{name}: {typ},\n", acc=acc, name=name(f), typ=t));
                    },
                    "Pointer" => {
                        // TODO: proper handling of pointers
                        let typ = if allow_multiples(&f) { "Vec<u32>" } else { "u32" };
                        fun.push_str(&format!("    pub {name}: {typ},\n", name=name(f), typ=typ));
                    },
                    "WriteOrder" => (),
                    _ => panic!("unexpected element {} under Entity", f.name),
                }
            }

            fun.push_str("}\n");
            fun.push_str("\n");

            // implementation
            fun.push_str(&format!("impl Default for {typ} {{\n", typ=name(c)));
            fun.push_str(&format!("    fn default() -> {typ} {{\n", typ=name(c)));
            fun.push_str(&format!("        {typ} {{\n", typ=name(c)));
            for f in &c.children {
                match &*f.name {
                    "Field" => {
                        fun.push_str(&format!("            {name}: {val},\n", name=name(f), val=default_value(&f)));
                    },
                    "Pointer" => {
                        // TODO: proper handling of pointers
                        let val = if allow_multiples(&f) { "vec![]" } else { "0" };
                        fun.push_str(&format!("            {name}: {val},\n", name=name(f), val=val));
                    },
                    "WriteOrder" => (),
                    _ => panic!("unexpected element {} under Entity", f.name),
                }
            }

            fun.push_str("        }\n");
            fun.push_str("    }\n");
            fun.push_str("}\n");
            fun.push_str("\n");

            fun.push_str(&format!("impl {typ} {{\n", typ=name(c)));
            fun.push_str("    pub fn new() -> Self {\n");
            fun.push_str("        Default::default()\n");
            fun.push_str("    }\n");

            // flags
            generate_flags_methods(fun, &c);

            fun.push_str("}\n");
            fun.push_str("\n");
        }
    }
}

fn generate_flags_methods(fun: &mut String, element: &Element) {
    for field in &element.children {
        if field.name == "Field" {
            for flag in &field.children {
                if flag.name == "Flag" {
                    let flag_name = name(&flag);
                    let mask = attr(&flag, "Mask");
                    fun.push_str(&format!("    pub fn get_{name}(&self) -> bool {{\n", name=flag_name));
                    fun.push_str(&format!("        self.{name} & {mask} != 0\n", name=name(&field), mask=mask));
                    fun.push_str("    }\n");
                    fun.push_str(&format!("    pub fn set_{name}(&mut self, val: bool) {{\n", name=flag_name));
                    fun.push_str("        if val {\n");
                    fun.push_str(&format!("            self.{name} |= {mask};\n", name=name(&field), mask=mask));
                    fun.push_str("        }\n");
                    fun.push_str("        else {\n");
                    fun.push_str(&format!("            self.{name} &= !{mask};\n", name=name(&field), mask=mask));
                    fun.push_str("        }\n");
                    fun.push_str("    }\n");
                }
            }
        }
    }
}

fn generate_is_supported_on_version(fun: &mut String, element: &Element) {
    fun.push_str("    pub fn is_supported_on_version(&self, version: &AcadVersion) -> bool {\n");
    fun.push_str("        match self {\n");
    for entity in &element.children {
        if name(&entity) != "Entity" && name(&entity) != "DimensionBase" && attr(&entity, "BaseClass") != "DimensionBase" {
            // TODO: support dimensions
            let mut predicates = vec![];
            if !min_version(&entity).is_empty() {
                predicates.push(format!("*version >= AcadVersion::{}", min_version(&entity)));
            }
            if !max_version(&entity).is_empty() {
                predicates.push(format!("*version <= AcadVersion::{}", max_version(&entity)));
            }
            let predicate = if predicates.len() == 0 { String::from("true") } else { predicates.join(" && ") };
            fun.push_str(&format!("            &EntityType::{typ}(_) => {{ {predicate} }},\n", typ=name(&entity), predicate=predicate));
        }
    }
    fun.push_str("        }\n");
    fun.push_str("    }\n");
}

fn generate_type_string(fun: &mut String, element: &Element) {
    fun.push_str("    pub fn from_type_string(type_string: &str) -> Option<EntityType> {\n");
    fun.push_str("        match type_string {\n");
    for c in &element.children {
        if name(c) != "Entity" && name(c) != "DimensionBase" && !attr(&c, "TypeString").is_empty() {
            let type_string = attr(&c, "TypeString");
            let type_strings = type_string.split(',').collect::<Vec<_>>();
            for t in type_strings {
                fun.push_str(&format!("            \"{type_string}\" => Some(EntityType::{typ}({typ}::new())),\n", type_string=t, typ=name(c)));
            }
        }
    }

    fun.push_str("            _ => None,\n");
    fun.push_str("        }\n");
    fun.push_str("    }\n");

    fun.push_str("    pub fn to_type_string(&self) -> &str {\n");
    fun.push_str("        match self {\n");
    for c in &element.children {
        // only write the first type string given
        let type_string = attr(&c, "TypeString");
        let type_strings = type_string.split(',').collect::<Vec<_>>();
        if name(c) != "Entity" && name(c) != "DimensionBase" && !type_string.is_empty() {
            fun.push_str(&format!("            &EntityType::{typ}(_) => {{ \"{type_string}\" }},\n", typ=name(c), type_string=type_strings[0]));
        }
    }
    fun.push_str("        }\n");
    fun.push_str("    }\n");
}

fn generate_try_apply_code_pair(fun: &mut String, element: &Element) {
    fun.push_str("    pub fn try_apply_code_pair(&mut self, pair: &CodePair) -> io::Result<bool> {\n");
    fun.push_str("        match self {\n");
    for c in &element.children {
        if c.name != "Entity" { panic!("expected top level entity"); }
        if name(c) != "Entity" && name(c) != "DimensionBase" && attr(&c, "BaseClass") != "DimensionBase" {
            if generate_reader_function(&c) {
                // TODO: handle dimensions
                // TODO: handle complex subtypes: e.g., lwpolyline has vertices
                let ent = if name(&c) == "Seqend" { "_ent" } else { "ent" }; // SEQEND doesn't use this variable
                fun.push_str(&format!("            &mut EntityType::{typ}(ref mut {ent}) => {{\n", typ=name(c), ent=ent));
                fun.push_str("                match pair.code {\n");
                let mut seen_codes = HashSet::new();
                for f in &c.children {
                    if f.name == "Field" && generate_reader(&f) {
                        for (i, &cd) in codes(&f).iter().enumerate() {
                            if !seen_codes.contains(&cd) {
                                seen_codes.insert(cd); // TODO: allow for duplicates
                                let reader = get_field_reader(&f);
                                let codes = codes(&f);
                                let write_cmd = match codes.len() {
                                    1 => {
                                        let read_fun = if allow_multiples(&f) {
                                            format!(".push({})", reader)
                                        }
                                        else {
                                            format!(" = {}", reader)
                                        };
                                        format!("ent.{field}{read_fun}", field=name(&f), read_fun=read_fun)
                                    },
                                    _ => {
                                        let suffix = match i {
                                            0 => "x",
                                            1 => "y",
                                            2 => "z",
                                            _ => panic!("impossible"),
                                        };
                                        format!("ent.{field}.{suffix} = {reader}", field=name(&f), suffix=suffix, reader=reader)
                                    }
                                };
                                fun.push_str(&format!("                    {code} => {{ {cmd}; }},\n", code=cd, cmd=write_cmd));
                            }
                        }
                    }
                    else if f.name == "Pointer" {
                        // TODO: proper handling of pointers
                        if allow_multiples(&f) {
                            fun.push_str(&format!("                    {code} => {{ ent.{field}.push(try!(as_u32(string_value(&pair.value)))); }},\n", code=code(&f), field=name(&f)));
                        }
                        else {
                            fun.push_str(&format!("                    {code} => {{ ent.{field} = try!(as_u32(string_value(&pair.value))); }},\n", code=code(&f), field=name(&f)));
                        }
                    }
                }

                fun.push_str("                    _ => return Ok(false),\n");
                fun.push_str("                }\n");
                fun.push_str("            },\n");
            }
            else {
                fun.push_str(&format!("            &mut EntityType::{typ}(_) => {{ panic!(\"this case should have been covered in a custom reader\"); }},\n", typ=name(&c)));
            }
        }
    }

    fun.push_str("        }\n");
    fun.push_str("        return Ok(true);\n");
    fun.push_str("    }\n");
}

fn generate_write(fun: &mut String, element: &Element) {
    fun.push_str("    pub fn write<T>(&self, common: &EntityCommon, version: &AcadVersion, writer: &mut CodePairAsciiWriter<T>) -> io::Result<()>\n");
    fun.push_str("        where T: Write {\n");
    fun.push_str("        match self {\n");
    for entity in &element.children {
        if name(&entity) != "Entity" && name(&entity) != "DimensionBase" && attr(&entity, "BaseClass") != "DimensionBase" {
            let ent = if name(&entity) == "Seqend" { "_ent" } else { "ent" }; // SEQEND doesn't use this variable
            fun.push_str(&format!("            &EntityType::{typ}(ref {ent}) => {{\n", typ=name(&entity), ent=ent));
            for line in generate_write_code_pairs(&entity) {
                fun.push_str(&format!("                {}\n", line));
            }

            fun.push_str("            },\n");
        }
    }
    fun.push_str("        }\n");
    fun.push_str("\n");
    fun.push_str("        Ok(())\n");
    fun.push_str("    }\n");
}

fn get_field_with_name<'a>(entity: &'a Element, field_name: &String) -> &'a Element {
    for field in &entity.children {
        if name(&field) == *field_name {
            return field;
        }
    }

    panic!("unable to find field {}", field_name);
}

fn generate_write_code_pairs(entity: &Element) -> Vec<String> {
    let mut commands = vec![];
    for f in &entity.children {
        if f.name == "WriteOrder" {
            // order was specifically given to us
            for write_command in &f.children {
                for line in generate_write_code_pairs_for_write_order(&entity, &write_command) {
                    commands.push(line);
                }
            }
            return commands;
        }
    }

    // no order given, use declaration order
    let subclass = attr(&entity, "SubclassMarker");
    if !subclass.is_empty() {
        commands.push(format!("try!(writer.write_code_pair(&CodePair::new_str(100, \"{subclass}\")));", subclass=subclass));
    }
    for field in &entity.children {
        if generate_writer(&field) {
            match &*field.name {
                "Field" => {
                    for line in get_write_lines_for_field(&field, vec![]) {
                        commands.push(line);
                    }
                },
                "Pointer" => { panic!("not used"); },
                _ => panic!("unexpected item {} in entity", field.name),
            }
        }
    }
    return commands;
}

fn generate_write_code_pairs_for_write_order(entity: &Element, write_command: &Element) -> Vec<String> {
    let mut commands = vec![];
    match &*write_command.name {
        "WriteField" => {
            let field_name = write_command.attributes.get("Field").unwrap();
            let field = get_field_with_name(&entity, &field_name);
            let mut write_conditions = vec![attr(&write_command, "WriteCondition")];
            if !attr(&write_command, "DontWriteIfValueIs").is_empty() {
                write_conditions.push(format!("ent.{} != {}", field_name, attr(&write_command, "DontWriteIfValueIs")));
            }
            for line in get_write_lines_for_field(&field, write_conditions) {
                commands.push(line);
            }
        },
        "WriteSpecificValue" => {
            let mut predicates = vec![];
            if !min_version(&write_command).is_empty() {
                predicates.push(format!("*version >= AcadVersion::{}", min_version(&write_command)));
            }
            if !max_version(&write_command).is_empty() {
                predicates.push(format!("*version <= AcadVersion::{}", max_version(&write_command)));
            }
            if !attr(&write_command, "DontWriteIfValueIs").is_empty() {
                predicates.push(format!("{} != {}", attr(&write_command, "Value"), attr(&write_command, "DontWriteIfValueIs")));
            }
            let code = code(&write_command);
            let expected_type = get_expected_type(code).unwrap();
            let typ = get_code_pair_type(expected_type);
            if predicates.len() > 0 {
                commands.push(format!("if {} {{", predicates.join(" && ")));
            }
            let indent = if predicates.len() > 0 { "    " } else { "" };
            commands.push(format!("{indent}try!(writer.write_code_pair(&CodePair::new_{typ}({code}, {val})));", indent=indent, typ=typ, code=code, val=attr(&write_command, "Value")));
            if predicates.len() > 0 {
                commands.push(String::from("}"));
            }
        },
        "Foreach" => {
            commands.push(format!("for item in &{} {{", attr(&write_command, "Field")));
            for write_command in &write_command.children {
                for line in generate_write_code_pairs_for_write_order(&entity, &write_command) {
                    commands.push(format!("    {}", line));
                }
            }
            commands.push(String::from("}"));
        },
        "WriteExtensionData" => {
            // TODO:
        },
        _ => panic!("unexpected write command {}", write_command.name),
    }

    commands
}

fn get_write_lines_for_field(field: &Element, write_conditions: Vec<String>) -> Vec<String> {
    let mut commands = vec![];
    let mut predicates = vec![];
    if !min_version(&field).is_empty() {
        predicates.push(format!("*version >= AcadVersion::{}", min_version(&field)));
    }
    if !max_version(&field).is_empty() {
        predicates.push(format!("*version <= AcadVersion::{}", max_version(&field)));
    }
    if disable_writing_default(&field) {
        predicates.push(format!("ent.{field} != {default}", field=name(&field), default=default_value(&field)));
    }
    for wc in write_conditions {
        if !wc.is_empty() {
            predicates.push(wc);
        }
    }
    let indent = if predicates.len() == 0 { "" } else { "    " };
    if predicates.len() > 0 {
        commands.push(format!("if {} {{", predicates.join(" && ")));
    }

    if allow_multiples(&field) {
        let expected_type = get_expected_type(codes(&field)[0]).unwrap();
        let val = if field.name == "Pointer" {
            "&as_handle(*v)"
        }
        else {
            if expected_type == ExpectedType::Str {
                "&v"
            }
            else {
                "*v"
            }
        };
        let typ = get_code_pair_type(expected_type);
        commands.push(format!("{indent}for v in &ent.{field} {{", indent=indent, field=name(&field)));
        commands.push(format!("{indent}    try!(writer.write_code_pair(&CodePair::new_{typ}({code}, {val})));", indent=indent, typ=typ, code=codes(&field)[0], val=val));
        commands.push(format!("{indent}}}", indent=indent));
    }
    else {
        for command in get_code_pairs_for_field(&field) {
            commands.push(format!("{indent}try!(writer.write_code_pair(&{command}));", indent=indent, command=command));
        }
    }

    if predicates.len() > 0 {
        commands.push(String::from("}"));
    }

    commands
}

fn get_code_pairs_for_field(field: &Element) -> Vec<String> {
    let codes = codes(&field);
    match codes.len() {
        1 => {
            vec![get_code_pair_for_field_and_code(codes[0], &field, None)]
        },
        _ => {
            let mut pairs = vec![];
            for (i, &cd) in codes.iter().enumerate() {
                let suffix = match i {
                    0 => "x",
                    1 => "y",
                    2 => "z",
                    _ => panic!("unexpected multiple codes"),
                };
                pairs.push(get_code_pair_for_field_and_code(cd, &field, Some(suffix)));
            }
            pairs
        }
    }
}

fn get_code_pair_for_field_and_code(code: i32, field: &Element, suffix: Option<&str>) -> String {
    let expected_type = get_expected_type(code).unwrap();
    let typ = get_code_pair_type(expected_type);
    let mut write_converter = attr(&field, "WriteConverter");
    if field.name == "Pointer" {
        write_converter = String::from("&as_handle({})");
    }
    if write_converter.is_empty() {
        if typ == "string" {
            write_converter = String::from("&{}");
        }
        else {
            write_converter = String::from("{}");
        }
    }
    let mut field_access = format!("ent.{field}", field=name(&field));
    if let Some(suffix) = suffix {
        field_access = format!("{}.{}", field_access, suffix);
    }
    let writer = write_converter.replace("{}", &field_access);
    format!("CodePair::new_{typ}({code}, {writer})", typ=typ, code=code, writer=writer)
}

fn load_xml() -> Element {
    let file = File::open("spec/EntitiesSpec.xml").unwrap();
    let file = BufReader::new(file);
    Element::parse(file).unwrap()
}

fn typ(element: &Element) -> String {
    attr(element, "Type")
}

fn generate_reader_function(element: &Element) -> bool {
    attr(&element, "GenerateReaderFunction") != "false"
}
