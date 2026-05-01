#!/usr/bin/python3
import json
import os

import numpy as np
from matplotlib import pyplot as plt


def avg(recordings):
    return sum(x["time"] for x in recordings) / len(recordings)


for file in ["manetsim.json", "ns3.json"]:
    data = json.load(open(f"benchmarks/{file}"))
    name = file.replace(".json", "")

    runtimes = []
    node_counts = sorted(map(lambda x: int(x), data.keys()))
    for node_count in node_counts:
        runtimes.append(avg(data[str(node_count)]))

    plt.plot(node_counts, runtimes, 'x-', label=name)

plt.title("Static Flood Routing with Friis Propagation (10 ticks)")
plt.xlabel("Network nodes")
plt.ylabel("Runtime (s)")
plt.legend()
ax = plt.gca()
# ax.set_aspect('equal')
# plt.show()
os.makedirs("graphs", exist_ok=True)
plt.savefig("graphs/node_comparison.png", dpi=1000)
plt.close()


for file in ["manetsim.json", "ns3.json"]:
    data = json.load(open(f"benchmarks/{file}"))
    name = file.replace(".json", "")

    packet_counts = []
    runtimes = []
    for results in data.values():
        packet_counts.append(int(results[0]["packets"]))
        runtimes.append(avg(results))
        print(f"{name}: ({results[0]['packets']}, {avg(results)})")

    plt.plot(packet_counts, runtimes, 'x-', label=name)

plt.title("Static Flood Routing with Friis Propagation (10 ticks)")
plt.xlabel("Packet count")
plt.ylabel("Runtime")
plt.legend()
ax = plt.gca()
# ax.set_aspect('equal')
# plt.show()
os.makedirs("graphs", exist_ok=True)
plt.savefig("graphs/packet_count_comparison.png")
plt.close()

data = json.load(open("benchmarks/ns3.json"))
node_counts = []
packet_counts = []
for node_count, results in data.items():
    node_counts.append(int(node_count))
    packet_counts.append(int(results[0]["packets"]))

plt.plot(node_counts, packet_counts, 'x-')
plt.title("Flood Routing Traffic")
plt.xlabel("Nodes")
plt.ylabel("Packets")
plt.savefig("graphs/flood_packets.png")
plt.close()

data = json.load(open("benchmarks/manetsim-scale.json"))
node_counts = []
runtimes = []
for node_count, results in data.items():
    node_counts.append(int(node_count))
    runtimes.append(avg(results))

plt.plot(node_counts, runtimes, 'x-')
ax = plt.gca()
ax.set_xticks([0, 500000, 1000000, 1500000, 2000000], ['0', '500k', '1M', '1.5M', '2M'])
plt.title("Random Walk Flood Routing with Simple Distance Propagation (100 ticks)")
plt.xlabel("Network nodes")
plt.ylabel("Runtime (s)")
plt.savefig("graphs/scalability.png", dpi=1000)
