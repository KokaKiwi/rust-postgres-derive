#![feature(plugin, custom_derive)]
#![plugin(postgres_derive_macros)]

#[macro_use]
extern crate postgres;

use postgres::{Connection, SslMode};
use postgres::error::Error;
use postgres::types::WrongType;

mod util;

#[test]
fn defaults() {
    #[derive(FromSql, ToSql, Debug, PartialEq)]
    struct InventoryItem {
        name: String,
        supplier_id: i32,
        price: Option<f64>,
    }

    let conn = Connection::connect("postgres://postgres@localhost", SslMode::None).unwrap();
    conn.batch_execute("CREATE TYPE pg_temp.\"InventoryItem\" AS (
                            name TEXT,
                            supplier_id INT,
                            price DOUBLE PRECISION
                        );").unwrap();

    let item = InventoryItem {
        name: "foobar".to_owned(),
        supplier_id: 100,
        price: Some(15.50),
    };

    let item_null = InventoryItem {
        name: "foobar".to_owned(),
        supplier_id: 100,
        price: None,
    };

    util::test_type(&conn, "\"InventoryItem\"",
                    &[(item, "ROW('foobar', 100, 15.50)"),
                      (item_null, "ROW('foobar', 100, NULL)")]);
}

#[test]
fn name_overrides() {
    #[derive(FromSql, ToSql, Debug, PartialEq)]
    #[postgres(name = "inventory_item")]
    struct InventoryItem {
        #[postgres(name = "name")]
        _name: String,
        #[postgres(name = "supplier_id")]
        _supplier_id: i32,
        #[postgres(name = "price")]
        _price: Option<f64>,
    }

    let conn = Connection::connect("postgres://postgres@localhost", SslMode::None).unwrap();
    conn.batch_execute("CREATE TYPE pg_temp.inventory_item AS (
                            name TEXT,
                            supplier_id INT,
                            price DOUBLE PRECISION
                        );").unwrap();

    let item = InventoryItem {
        _name: "foobar".to_owned(),
        _supplier_id: 100,
        _price: Some(15.50),
    };

    let item_null = InventoryItem {
        _name: "foobar".to_owned(),
        _supplier_id: 100,
        _price: None,
    };

    util::test_type(&conn, "inventory_item",
                    &[(item, "ROW('foobar', 100, 15.50)"),
                      (item_null, "ROW('foobar', 100, NULL)")]);
}

#[test]
fn wrong_name() {
    #[derive(FromSql, ToSql, Debug, PartialEq)]
    struct InventoryItem {
        name: String,
        supplier_id: i32,
        price: Option<f64>,
    }

    let conn = Connection::connect("postgres://postgres@localhost", SslMode::None).unwrap();
    conn.batch_execute("CREATE TYPE pg_temp.inventory_item AS (
                            name TEXT,
                            supplier_id INT,
                            price DOUBLE PRECISION
                        );").unwrap();

    let item = InventoryItem {
        name: "foobar".to_owned(),
        supplier_id: 100,
        price: Some(15.50),
    };

    match conn.execute("SELECT $1::inventory_item", &[&item]) {
        Err(Error::Conversion(ref r)) if r.is::<WrongType>() => {}
        v => panic!("unexpected response {:?}", v),
    }
}

#[test]
fn extra_field() {
    #[derive(FromSql, ToSql, Debug, PartialEq)]
    #[postgres(name = "inventory_item")]
    struct InventoryItem {
        name: String,
        supplier_id: i32,
        price: Option<f64>,
        foo: i32,
    }

    let conn = Connection::connect("postgres://postgres@localhost", SslMode::None).unwrap();
    conn.batch_execute("CREATE TYPE pg_temp.inventory_item AS (
                            name TEXT,
                            supplier_id INT,
                            price DOUBLE PRECISION
                        );").unwrap();

    let item = InventoryItem {
        name: "foobar".to_owned(),
        supplier_id: 100,
        price: Some(15.50),
        foo: 0,
    };

    match conn.execute("SELECT $1::inventory_item", &[&item]) {
        Err(Error::Conversion(ref r)) if r.is::<WrongType>() => {}
        v => panic!("unexpected response {:?}", v),
    }
}

#[test]
fn missing_field() {
    #[derive(FromSql, ToSql, Debug, PartialEq)]
    #[postgres(name = "inventory_item")]
    struct InventoryItem {
        name: String,
        supplier_id: i32,
    }

    let conn = Connection::connect("postgres://postgres@localhost", SslMode::None).unwrap();
    conn.batch_execute("CREATE TYPE pg_temp.inventory_item AS (
                            name TEXT,
                            supplier_id INT,
                            price DOUBLE PRECISION
                        );").unwrap();

    let item = InventoryItem {
        name: "foobar".to_owned(),
        supplier_id: 100,
    };

    match conn.execute("SELECT $1::inventory_item", &[&item]) {
        Err(Error::Conversion(ref r)) if r.is::<WrongType>() => {}
        v => panic!("unexpected response {:?}", v),
    }
}

#[test]
fn wrong_type() {
    #[derive(FromSql, ToSql, Debug, PartialEq)]
    #[postgres(name = "inventory_item")]
    struct InventoryItem {
        name: String,
        supplier_id: i32,
        price: i32,
    }

    let conn = Connection::connect("postgres://postgres@localhost", SslMode::None).unwrap();
    conn.batch_execute("CREATE TYPE pg_temp.inventory_item AS (
                            name TEXT,
                            supplier_id INT,
                            price DOUBLE PRECISION
                        );").unwrap();

    let item = InventoryItem {
        name: "foobar".to_owned(),
        supplier_id: 100,
        price: 0,
    };

    match conn.execute("SELECT $1::inventory_item", &[&item]) {
        Err(Error::Conversion(ref r)) if r.is::<WrongType>() => {}
        v => panic!("unexpected response {:?}", v),
    }
}
