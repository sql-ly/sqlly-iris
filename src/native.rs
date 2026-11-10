use crate::{
    Client, Result, Value,
    protocol::{ListItem, Operation, encode_item, encode_string},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalData {
    pub exists: bool,
    pub has_children: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalDirection {
    Next = 3,
    Previous = 7,
}

impl Client {
    pub async fn global_get(&mut self, global: &str, subscripts: &[Value]) -> Result<Value> {
        let payload = global_payload(global, subscripts);
        let response = self
            .native_operation(Operation::GlobalGet, &payload)
            .await?;
        if response.status == 0xcdf1 {
            return Ok(Value::Null);
        }
        scalar_value(response.payload.as_slice())
    }

    pub async fn global_set(
        &mut self,
        global: &str,
        value: &Value,
        subscripts: &[Value],
    ) -> Result<()> {
        let mut payload = global_payload(global, subscripts);
        encode_item(&mut payload, value);
        self.native_operation(Operation::GlobalSet, &payload)
            .await
            .map(drop)
    }

    pub async fn global_kill(&mut self, global: &str, subscripts: &[Value]) -> Result<()> {
        let payload = global_payload(global, subscripts);
        self.native_operation(Operation::GlobalKill, &payload)
            .await
            .map(drop)
    }

    pub async fn global_data(&mut self, global: &str, subscripts: &[Value]) -> Result<GlobalData> {
        let mut payload = global_payload(global, subscripts);
        encode_count(&mut payload, 0);
        let response = self
            .native_operation(Operation::GlobalData, &payload)
            .await?;
        let mut input = response.payload.as_slice();
        let flags = crate::client::read_i64(&mut input)?;
        Ok(GlobalData {
            exists: flags % 10 == 1,
            has_children: flags >= 10,
        })
    }

    pub async fn global_order(
        &mut self,
        global: &str,
        current: &str,
        subscripts: &[Value],
        direction: GlobalDirection,
    ) -> Result<Option<String>> {
        let mut payload = Vec::new();
        encode_string(&mut payload, global);
        encode_count(&mut payload, subscripts.len() as i64 + 1);
        for subscript in subscripts {
            encode_item(&mut payload, subscript);
        }
        encode_string(&mut payload, current);
        encode_count(&mut payload, direction as i64);
        let response = self
            .native_operation(Operation::GlobalOrder, &payload)
            .await?;
        let mut input = response.payload.as_slice();
        let value = ListItem::parse(&mut input)?.as_string()?;
        Ok((!value.is_empty()).then_some(value))
    }

    pub async fn class_method(
        &mut self,
        class: &str,
        method: &str,
        arguments: &[Value],
    ) -> Result<Value> {
        let payload = method_payload(class, method, arguments);
        let response = self
            .native_operation(Operation::ClassMethodValue, &payload)
            .await?;
        scalar_value(response.payload.as_slice())
    }

    pub async fn class_method_void(
        &mut self,
        class: &str,
        method: &str,
        arguments: &[Value],
    ) -> Result<()> {
        let payload = method_payload(class, method, arguments);
        self.native_operation(Operation::ClassMethodVoid, &payload)
            .await
            .map(drop)
    }

    pub async fn method(
        &mut self,
        object_ref: &str,
        method: &str,
        arguments: &[Value],
    ) -> Result<Value> {
        let payload = instance_method_payload(object_ref, method, arguments);
        let response = self
            .native_operation(Operation::MethodValue, &payload)
            .await?;
        scalar_value(response.payload.as_slice())
    }

    pub async fn method_void(
        &mut self,
        object_ref: &str,
        method: &str,
        arguments: &[Value],
    ) -> Result<()> {
        let payload = instance_method_payload(object_ref, method, arguments);
        self.native_operation(Operation::MethodVoid, &payload)
            .await
            .map(drop)
    }

    pub async fn property_get(&mut self, object_ref: &str, property: &str) -> Result<Value> {
        let mut payload = Vec::new();
        encode_item(&mut payload, &Value::ObjectRef(object_ref.into()));
        encode_string(&mut payload, property);
        let response = self
            .native_operation(Operation::PropertyGet, &payload)
            .await?;
        scalar_value(response.payload.as_slice())
    }

    pub async fn property_set(
        &mut self,
        object_ref: &str,
        property: &str,
        value: &Value,
    ) -> Result<()> {
        let mut payload = Vec::new();
        encode_item(&mut payload, &Value::ObjectRef(object_ref.into()));
        encode_string(&mut payload, property);
        encode_item(&mut payload, value);
        self.native_operation(Operation::PropertySet, &payload)
            .await
            .map(drop)
    }
}

fn instance_method_payload(object_ref: &str, method: &str, arguments: &[Value]) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_item(&mut payload, &Value::ObjectRef(object_ref.into()));
    encode_string(&mut payload, method);
    encode_count(&mut payload, arguments.len() as i64);
    for argument in arguments {
        encode_item(&mut payload, argument);
    }
    payload
}

fn global_payload(global: &str, subscripts: &[Value]) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_string(&mut payload, global);
    encode_count(&mut payload, subscripts.len() as i64);
    for subscript in subscripts {
        encode_item(&mut payload, subscript);
    }
    payload
}

fn method_payload(name: &str, method: &str, arguments: &[Value]) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_string(&mut payload, name);
    encode_string(&mut payload, method);
    encode_count(&mut payload, arguments.len() as i64);
    for argument in arguments {
        encode_item(&mut payload, argument);
    }
    payload
}

fn scalar_value(payload: &[u8]) -> Result<Value> {
    let mut input = payload;
    if input.is_empty() {
        return Ok(Value::Null);
    }
    ListItem::parse(&mut input)?.to_value()
}

fn encode_count(output: &mut Vec<u8>, value: i64) {
    encode_item(output, &Value::I64(value));
}
