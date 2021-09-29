# test-dht

This repository is use to connect node and store and get values from and to DHT. For that you need to use below commands.

run the code using below command

```
cargo run --release
```

when app will run, you can use below command to connect the node, put/get data to dht.

1. Add new Node

```
ADD_NODE <PEER_ID> <TCP Url>
```

This command will use to join the node with current node with ip4 url like "/ip4/127.0.0.1/tcp/57165"

2. Put value to DHT

```
PUT <Key> <Value>
```

This command will use to put value inside the DHT.

3. Get value from DHT

```
GET <Key>
```

This command will use to put value inside the DHT.

While you are work over the network, not on local network
-------------------------------
When you are on different machines, your tcp url will change something like

```
/ip4/<IP Address>/tcp/<Port>
```

IP Address:- You can get this IP using ```ifconfig``` command
