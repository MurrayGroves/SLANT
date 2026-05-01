#include "ns3/applications-module.h"
#include "ns3/core-module.h"
#include "ns3/internet-module.h"
#include "ns3/mobility-module.h"
#include "ns3/network-module.h"
#include "ns3/propagation-module.h"
#include <cmath>
#include <iostream>
#include <set>
#include <vector>
#define _USE_MATH_DEFINES
#include <cmath>
#include <cstdlib>
#include <map>
#include <math.h>
#include <string>

using namespace ns3;

NS_LOG_COMPONENT_DEFINE("FloodRoutingExample");

// --- Forward Declaration ---
class FloodApp;

// --- Custom Packet Header ---
class RawFloodHeader : public Header {
public:
  RawFloodHeader() : m_src(0), m_seq(0), m_hops(0), m_content(0) {}
  virtual ~RawFloodHeader() {}

  void SetSrc(uint32_t src) { m_src = src; }
  void SetSeq(uint32_t seq) { m_seq = seq; }
  void SetHops(uint8_t hops) { m_hops = hops; }
  void SetContent(uint32_t content) { m_content = content; }

  uint32_t GetSrc() const { return m_src; }
  uint32_t GetSeq() const { return m_seq; }
  uint8_t GetHops() const { return m_hops; }
  uint32_t GetContent() const { return m_content; }

  static TypeId GetTypeId(void) {
    static TypeId tid = TypeId("ns3::RawFloodHeader")
                            .SetParent<Header>()
                            .AddConstructor<RawFloodHeader>();
    return tid;
  }
  virtual TypeId GetInstanceTypeId(void) const { return GetTypeId(); }
  virtual uint32_t GetSerializedSize(void) const { return 13; }
  virtual void Serialize(Buffer::Iterator start) const {
    start.WriteHtonU32(m_src);
    start.WriteHtonU32(m_seq);
    start.WriteU8(m_hops);
    start.WriteHtonU32(m_content);
  }
  virtual uint32_t Deserialize(Buffer::Iterator start) {
    m_src = start.ReadNtohU32();
    m_seq = start.ReadNtohU32();
    m_hops = start.ReadU8();
    m_content = start.ReadNtohU32();
    return GetSerializedSize();
  }
  virtual void Print(std::ostream &os) const {
    os << "Src=" << m_src << ", Seq=" << m_seq;
  }

private:
  uint32_t m_src;
  uint32_t m_seq;
  uint8_t m_hops;
  uint32_t m_content;
};

// --- Custom Wireless Channel with Friis and Pruning ---
class RawWirelessChannel : public Object {
public:
  static TypeId GetTypeId(void) {
    static TypeId tid = TypeId("ns3::RawWirelessChannel")
                            .SetParent<Object>()
                            .SetGroupName("Network")
                            .AddConstructor<RawWirelessChannel>();
    return tid;
  }

  RawWirelessChannel();
  virtual ~RawWirelessChannel() = default;

  void SetTxPower(double p) { m_txPower = p; }
  void SetTxGain(double g) { m_txGain = g; }
  void SetRxGain(double g) { m_rxGain = g; }
  void SetSensitivity(double s) { m_sensitivity = s; }

  // NOTE: Methods defined after FloodApp to resolve incomplete types
  void Attach(Ptr<FloodApp> app);
  void Send(Ptr<Packet> packet, uint32_t senderId);

private:
  std::vector<Ptr<FloodApp>> m_apps;
  std::map<uint32_t, Ptr<FloodApp>> m_appMap;
  Ptr<FriisPropagationLossModel> m_loss;
  double m_txPower{8.0};
  double m_txGain{11.0};
  double m_rxGain{0.0};
  double m_sensitivity{-90.0};
  double m_maxRangeSquared{0.0};
  double m_wavelength{0.34538301613};
};

NS_OBJECT_ENSURE_REGISTERED(RawWirelessChannel);

// --- Application ---
class FloodApp : public Application {
public:
  static TypeId GetTypeId(void) {
    static TypeId tid = TypeId("ns3::FloodApp")
                            .SetParent<Application>()
                            .AddConstructor<FloodApp>();
    return tid;
  }

  FloodApp() : m_id(0), m_seqCounter(0), m_totalReceived(0) {}
  virtual ~FloodApp() {}

  void SetNodeId(uint32_t id) { m_id = id; }
  uint32_t GetNodeId() const { return m_id; }
  uint32_t GetTotalReceived() const { return m_totalReceived; }
  uint32_t GetSeq() const { return m_seqCounter; }
  void SetChannel(Ptr<RawWirelessChannel> ch) { m_channel = ch; }

  void Receive(Ptr<Packet> packet) {
    RawFloodHeader header;
    packet->PeekHeader(header);

    m_totalReceived++;
    // Deduplication check
    auto key = std::make_pair(header.GetSrc(), header.GetSeq());
    if (m_seen.find(key) != m_seen.end())
      return;

    m_seen.insert(key);

    // Forward packet if hops remain
    if (header.GetHops() > 0) {
      Ptr<Packet> copy = packet->Copy();
      copy->RemoveHeader(header);
      header.SetHops(header.GetHops() - 1);
      copy->AddHeader(header);
      m_channel->Send(copy, m_id);
    }
  }

private:
  virtual void StartApplication(void) {
    m_seqCounter = 0;
    SendPacket();
  }

  virtual void StopApplication(void) { Simulator::Cancel(m_sendEvent); }

  void SendPacket() {
    Ptr<Packet> p = Create<Packet>();
    RawFloodHeader header;
    header.SetSrc(m_id);
    header.SetSeq(m_seqCounter++);
    header.SetHops(5);
    header.SetContent(m_id);
    p->AddHeader(header);

    m_channel->Send(p, m_id);
    m_sendEvent =
        Simulator::Schedule(Seconds(5.0), &FloodApp::SendPacket, this);
  }

  uint32_t m_id;
  uint32_t m_seqCounter;
  uint32_t m_totalReceived;
  EventId m_sendEvent;
  Ptr<RawWirelessChannel> m_channel;
  std::set<std::pair<uint32_t, uint32_t>> m_seen;
};

NS_OBJECT_ENSURE_REGISTERED(FloodApp);

// --- RawWirelessChannel Constructors ---
RawWirelessChannel::RawWirelessChannel() {
  m_loss = CreateObject<FriisPropagationLossModel>();

  // Parameters: TxPower=8dBm, TxGain=11dBm, RxGain=30dBm, MDS=-120dBm,
  // Freq=868MHz
  double txPower = 8.0;
  double txGain = 11.0;
  double rxGain = 30.0;
  double mds = -120.0;

  double pathLossLimit = txPower + txGain + rxGain - mds;

  double freq = 868e6;
  double freqTerm = 20 * std::log10(freq);
  double constTerm = 20 * std::log10(4 * M_PI / 3e8);

  double log10_d = (pathLossLimit - freqTerm - constTerm) / 20.0;
  double maxDistance = std::pow(10.0, log10_d);

  m_maxRangeSquared = maxDistance * maxDistance;

  m_loss->SetFrequency(freq);
}

// --- RawWirelessChannel Method Definitions ---

void RawWirelessChannel::Attach(Ptr<FloodApp> app) {
  m_apps.push_back(app);
  m_appMap[app->GetNodeId()] = app;
}

void RawWirelessChannel::Send(Ptr<Packet> packet, uint32_t senderId) {
  // Fast lookup for sender
  auto senderIt = m_appMap.find(senderId);
  if (senderIt == m_appMap.end())
    return;

  Ptr<MobilityModel> senderMob =
      senderIt->second->GetNode()->GetObject<MobilityModel>();
  if (senderMob == nullptr)
    return;

  double effectiveTxPower = m_txPower + m_txGain;

  // Iterate all receivers
  for (auto &app : m_apps) {
    if (app->GetNodeId() == senderId)
      continue;

    Ptr<MobilityModel> receiverMob = app->GetNode()->GetObject<MobilityModel>();

    // Optimized Prune: Check theoretical max range
    double distance = senderMob->GetDistanceFrom(receiverMob);

    if (distance * distance > m_maxRangeSquared) {
      continue;
    }

    // Run detailed Friis calculation
    // double rxPower = m_loss->CalcRxPower (effectiveTxPower, senderMob,
    // receiverMob) + m_rxGain;
    double rxPower =
        effectiveTxPower + m_rxGain +
        (20.0 * std::log10(m_wavelength / (4.0 * M_PI * distance)));

    if (rxPower >= m_sensitivity) {
      // double delay = distance / 3e8;
      //(*app).Receive(packet);
      Simulator::ScheduleWithContext(app->GetNode()->GetId(), Seconds(1.0),
                                     &FloodApp::Receive, app, packet->Copy());
    }
  }
}

// --- Progress Trace Callback ---
static void ProgressTrace(double totalTime) {
  double now = Simulator::Now().GetSeconds();
  uint8_t percent = 0;
  if (totalTime > 0) {
    percent = static_cast<uint8_t>(((now / totalTime) * 100));
  }
  std::cout << "Progress: " << +percent << "% (" << now << "s / " << totalTime
            << "s)..." << std::endl;

  if (now < totalTime) {
    Simulator::Schedule(Seconds(0.01), &ProgressTrace, totalTime);
  }
}

// --- Main ---
int main(int argc, char *argv[]) {
  struct timeval timestr;
  gettimeofday(&timestr, NULL);
  double start_time = timestr.tv_sec + (timestr.tv_usec / 1000000.0);
  const char *env_val = std::getenv("NUM_NODES");
  int num_nodes = std::stoi(env_val);
  uint32_t nNodes = num_nodes;
  int grid_width = static_cast<int>(std::sqrt(num_nodes));
  double distance = 3000.0;
  double simulationTime = 10.0;

  NodeContainer nodes;
  nodes.Create(nNodes);

  MobilityHelper mobility;
  mobility.SetPositionAllocator(
      "ns3::GridPositionAllocator", "MinX", DoubleValue(0.0), "MinY",
      DoubleValue(0.0), "DeltaX", DoubleValue(distance), "DeltaY",
      DoubleValue(distance), "GridWidth", UintegerValue(grid_width),
      "LayoutType", StringValue("RowFirst"));
  mobility.SetMobilityModel("ns3::ConstantPositionMobilityModel");
  mobility.Install(nodes);

  Ptr<RawWirelessChannel> channel = CreateObject<RawWirelessChannel>();

  std::vector<Ptr<FloodApp>> apps;

  for (uint32_t i = 0; i < nNodes; ++i) {
    Ptr<FloodApp> app = CreateObject<FloodApp>();
    nodes.Get(i)->AddApplication(app);
    app->SetChannel(channel);
    app->SetNodeId(i);
    app->SetStartTime(Seconds(0.0));
    app->SetStopTime(Seconds(simulationTime));
    channel->Attach(app);
    apps.push_back(app);
  }

  // Schedule Progress Tracking at start
  Simulator::Schedule(Seconds(0.0), &ProgressTrace, simulationTime);

  Simulator::Stop(Seconds(simulationTime));
  Simulator::Run();
  Simulator::Destroy();

  std::cout << "Simulation Complete." << std::endl;

  gettimeofday(&timestr, NULL);
  double end_time = timestr.tv_sec + (timestr.tv_usec / 1000000.0);

  std::cout << "Simulation took " << end_time - start_time << " secs"
            << std::endl;

  int total_received = 0;
  int total_originated = 0;
  for (uint32_t i = 0; i < apps.size(); ++i) {
    // std::cout << "Node " << i << ": " << apps[i]->GetTotalReceived () << "
    // received." << std::endl;
    total_received += apps[i]->GetTotalReceived();
    total_originated += apps[i]->GetSeq();
  }

  std::cout << "received " << total_received << " packets" << std::endl;
  std::cout << "originated " << total_originated << " packets" << std::endl;

  return 0;
}
