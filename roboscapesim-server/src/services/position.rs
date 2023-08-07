
pub fn create_position_service(id: &str) -> Service {
    // Create definition struct
    let mut definition = ServiceDefinition {
        id: id.to_owned(),
        methods: BTreeMap::new(),
        events: BTreeMap::new(),
        description: IoTScapeServiceDescription {
            description: Some("Get the position and orientation of an object".to_owned()),
            externalDocumentation: None,
            termsOfService: None,
            contact: Some("gstein@ltu.edu".to_owned()),
            license: None,
            version: "1".to_owned(),
        },
    };

    // Define methods
    definition.methods.insert(
        "getPosition".to_owned(),
        MethodDescription {
            documentation: Some("Get XYZ coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned(), "number".to_owned(), "number".to_owned()],
            },
        },
    );


    definition.methods.insert(
        "getX".to_owned(),
        MethodDescription {
            documentation: Some("Get X coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );


    definition.methods.insert(
        "getY".to_owned(),
        MethodDescription {
            documentation: Some("Get Y coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );


    definition.methods.insert(
        "getZ".to_owned(),
        MethodDescription {
            documentation: Some("Get Z coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "getHeading".to_owned(),
        MethodDescription {
            documentation: Some("Get heading direction (yaw) of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );
    
    let service = setup_service(definition, ServiceType::PositionSensor, None);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = Instant::now();
    let announce_period = Duration::from_secs(30);

    Service {
        service_type: ServiceType::World,
        service,
        last_announce,
        announce_period,
    }
}