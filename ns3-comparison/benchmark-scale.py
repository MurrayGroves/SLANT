#!/bin/python3
import subprocess
import os
import json

NUM_TRIALS = 3

def get_ns3_time(text: str) -> float:
    lines = text.splitlines()
    for line in lines:
        if line.startswith("Simulation took"):
            return float(line.split(" ")[2])


def get_manetsim_time(text: str) -> float:
    lines = text.splitlines()
    for line in lines:
        if line.startswith("test result"):
            time = float(line.split(" ")[-1].replace("s", ""))
            if time != 0:
                return time

def get_ns3_packets(text: str) -> int:
    lines = text.splitlines()
    for line in lines:
        if line.startswith("received "):
            return int(line.split(" ")[1])

def get_manetsim_packets(text: str) -> int:
    lines = text.splitlines()
    for line in lines:
        if line.endswith("packets total"):
            print(f"Found line {line}")
            return line.split(" ")[-3]

    print("Didn't find line")
    return "not found"

def run_trial_ns3(nodes: int):
    result = subprocess.run(["/software/spack/linux-rocky8-broadwell/gcc-12.3.0/apptainer-1.3.1-ksax/bin/apptainer", "run", "--writable-tmpfs", "--cwd", "/home/ns3-optimized/ns-3.41", "ns3-flood_latest.sif"], env={
        "NUM_NODES": str(nodes),
    }, stdout=subprocess.PIPE)

    out = result.stdout.decode()
    time = get_ns3_time(out)
    packets = get_ns3_packets(out)
    return (time, packets)

def run_trial_manetsim(nodes: int):
    result = subprocess.run(["/software/spack/linux-rocky8-broadwell/gcc-12.3.0/apptainer-1.3.1-ksax/bin/apptainer", "run", "--cwd", "/home/ij22909", "manetsim-scale_no-stats.sif"], env={
        "NUM_NODES": str(nodes),
    }, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    out = result.stdout.decode() + result.stderr.decode()
    time = get_manetsim_time(out)
    packets = get_manetsim_packets(out)
    print(f"Packets: {packets}")
    return (time, packets)

os.makedirs("benchmarks", exist_ok=True)

num_nodes = [2**i for i in range(1, 22)]

for nodes in num_nodes:
    for trial in range(NUM_TRIALS):
        res = run_trial_manetsim(nodes)
        try:
            old = json.load(open(f"benchmarks/manetsim-scale.json", "r"))
        except:
            old = {}

        old_array = old.get(str(nodes), [])
        old_array.append({"time": res[0], "packets": res[1]})
        old[nodes] = old_array

        json.dump(old, open(f"benchmarks/manetsim-scale.json", "w+"))
        print(f"Finished manetsim-{nodes} ({trial + 1}/{NUM_TRIALS})")

