FROM egiona/ns3-base:u22.04-n3.41

RUN /home/build-optimized.sh
WORKDIR /home/ns3-optimized/ns-3.41/
ADD flood.cc scratch
CMD ["./ns3", "run", "flood"]
