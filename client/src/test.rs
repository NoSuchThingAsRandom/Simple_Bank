fn test_check_messages_bench(&mut self) -> Vec<i64> {
    info!("Updating messages");
    let mut differences = Vec::new();
    for msg in self.messages_in_receiver.try_iter() {
        for client in &mut self.clients {
            if client.addr == msg.sender {
                let time: Result<i64, <i64 as FromStr>::Err> = msg.data.parse();
                match time {
                    Ok(t) => {
                        let rec_time: i64 = chrono::Utc::now().timestamp_millis();
                        println!("Time {} dif {}", rec_time, t);
                        println!("Received {}", rec_time - t);
                        differences.push(rec_time - t);
                    }
                    Err(_) => {
                        for fuck in msg.data.split_whitespace() {
                            let time: Result<i64, <i64 as FromStr>::Err> =
                                fuck[0..fuck.len()].parse();
                            if time.is_ok() {
                                let current = chrono::Utc::now().timestamp_millis();
                                let received = time.unwrap();
                                //println!("CUrrent {} Time Sent{}",current,receievd);
                                let different = current - received;
                                differences.push(different);
                            }
                        }
                    }
                }
            }
        }
    }
    differences
}

pub fn test_multi_server_multi_client(&mut self) {
    info!("Starting multi client multi server test");
    // Test consts
    const NUM_THREADS: u32 = 10;
    const NUM_INSTANCES: u32 = 120;
    let instance_per_thread = (NUM_INSTANCES / NUM_THREADS) as usize;
    const NUM_MESSAGES: u32 = 500;
    const PORT: u32 = 50000;

    let mut addresses = Vec::new();
    let mut current_thread_state: Vec<InputLoop> = Vec::new();
    let mut all_states: Vec<Vec<InputLoop>> = Vec::new();
    println!("Starting instances");
    //Start instances
    for instances_index in 0..NUM_INSTANCES {
        let mut host = String::from("127.0.0.1:");
        host.push_str((PORT + instances_index).to_string().as_str());
        let state = InputLoop::new(host.clone());
        addresses.push(host);
        if current_thread_state.len() == instance_per_thread {
            all_states.push(current_thread_state);
            current_thread_state = Vec::new();
        }
        current_thread_state.push(state);
    }
    all_states.push(current_thread_state);
    info!("Created instances");
    //Start threads
    let mut threads = Vec::new();
    for mut state in all_states {
        let address_copy: Vec<String> = addresses.clone();
        threads.push(thread::spawn(move || {
            //Connect to other clients
            for sub_state in &mut state {
                for address in &address_copy {
                    match sub_state.connect(String::from(address)) {
                        Some(c) => {
                            sub_state.clients.push(c);
                        }
                        None => {
                            println!("Failed to connect to client");
                        }
                    }
                }
            }
            println!("Created connections");
            thread::sleep(Duration::from_secs(2));
            for sub_state in &mut state {
                sub_state.update_clients();
            }

            //Send messages
            let mut differences: Vec<i64> = Vec::new();
            for _msg_index in 0..NUM_MESSAGES {
                for sub_state in &mut state {
                    for address in &address_copy {
                        let mut msg = chrono::Utc::now().timestamp_millis().to_string();
                        msg.push(' ');
                        sub_state.send_message(address.clone(), msg);
                    }
                    differences.append(&mut sub_state.test_check_messages_bench());
                }
            }

            //Calculate average time difference
            println!("Sent messages");
            thread::sleep(Duration::from_secs(10));
            for sub_state in &mut state {
                for timestamp in sub_state.test_check_messages_bench().iter() {
                    differences.push(timestamp - 10000);
                }
            }
            let mut total: i64 = 0;
            for dif in &differences {
                total = total + *dif as i64;
            }
            thread::sleep(Duration::from_secs(1));
            if total > 0 {
                total = total / differences.len() as i64;
                println!("Mean difference in millis (maybe?) {}", total);
            } else {
                println!("Time fucked up? {}", total);
            }
            info!("     Sent messages");
            total
        }));
    }
    let size = threads.len();
    let mut total = 0;
    for thread in threads {
        let score = thread.join().unwrap();
        if score > 0 {
            total += score;
        }
    }
    println!("\n\nTotal Average {}", total / size as i64);

    self.update_clients();
    info!("Finished");
}

pub fn test_single_server_multi_client(&mut self) {
    info!("Starting multi client multi server test");
    // Test consts
    const NUM_THREADS: u32 = 10;
    const NUM_INSTANCES: u32 = 120;
    let instance_per_thread = (NUM_INSTANCES / NUM_THREADS) as usize;
    const NUM_MESSAGES: u32 = 500;
    const PORT: u32 = 50000;

    let mut addresses = Vec::new();
    let mut current_thread_state: Vec<InputLoop> = Vec::new();
    let mut all_states: Vec<Vec<InputLoop>> = Vec::new();
    println!("Starting instances");
    //Start instances
    for instances_index in 0..NUM_INSTANCES {
        let mut host = String::from("127.0.0.1:");
        host.push_str((PORT + instances_index + 1).to_string().as_str());
        let state = InputLoop::new(host.clone());
        addresses.push(host);
        if current_thread_state.len() == instance_per_thread {
            all_states.push(current_thread_state);
            current_thread_state = Vec::new();
        }
        current_thread_state.push(state);
    }
    all_states.push(current_thread_state);

    //Start Server Thread
    thread::spawn(move || {
        let mut differences = Vec::new();
        let mut host = String::from("127.0.0.1:");
        host.push_str((PORT).to_string().as_str());
        let mut state = InputLoop::new(host);
        while state.clients.len() != NUM_INSTANCES as usize {
            state.update_clients();
        }
        //Calculate average time difference
        println!("Sent messages");

        for timestamp in state.test_check_messages_bench().iter() {
            differences.push(timestamp - 10000);
        }
        let mut total: i64 = 0;
        for dif in &differences {
            total = total + *dif as i64;
        }
        thread::sleep(Duration::from_secs(1));
        if total > 0 {
            total = total / differences.len() as i64;
            println!("Mean difference in millis (maybe?) {}", total);
        } else {
            println!("Time fucked up? {}", total);
        }
        info!("     Sent messages");
        println!("\n\nTotal Average {}", total);
    });

    thread::sleep(Duration::from_secs(5));
    info!("Created instances");
    //Start client threads
    let mut threads = Vec::new();
    for mut state in all_states {
        let _address_copy: Vec<String> = addresses.clone();
        threads.push(thread::spawn(move || {
            let mut host = String::from("127.0.0.1:");
            host.push_str((PORT).to_string().as_str());
            //Connect to server
            for sub_state in &mut state {
                match sub_state.connect(host.clone()) {
                    Some(c) => {
                        sub_state.clients.push(c);
                    }
                    None => {
                        println!("Failed to connect to client");
                    }
                }
            }
            println!("Created connections");
            thread::sleep(Duration::from_secs(5));

            //Send messages
            for _msg_index in 0..NUM_MESSAGES {
                for sub_state in &mut state {
                    let mut msg = chrono::Utc::now().timestamp_millis().to_string();
                    msg.push(' ');
                    sub_state.send_message(host.clone(), msg);
                }
            }
            state.get_mut(0);
        }));
    }

    //Stop client threads
    for thread in threads {
        thread.join().unwrap();
    }

    self.update_clients();
    info!("Finished");
}

pub fn fish(&mut self) {
    thread::sleep(Duration::from_secs(5));

    let client = self.connect(String::from("127.0.0.1:49999")).unwrap();
    self.clients.push(client);
    thread::sleep(Duration::from_secs(5));

    for value in String::from("ABCDEF").split("") {
        thread::sleep(Duration::from_secs(2));
        println!("{}", value);
        println!(
            "{}",
            self.send_message(String::from("127.0.0.1:49999"), value.to_string())
        );
    }
    //println!("{}", self.send_message(String::from("127.0.0.1:49999"), value.to_string()));

    thread::sleep(Duration::from_secs(15));
    thread::sleep(Duration::from_secs(1));
    self.shutdown();
    println!("Finished test");
}