require 'openc3/microservices/microservice'

module OmsCosmosPlugin
  class OmsBridgeMicroservice < OpenC3::Microservice
    def run
      binary = ENV.fetch('OMS_BRIDGE_BINARY', 'oms_bridge')
      pid = nil
      loop do
        pid = spawn(ENV.to_h, binary)
        Process.wait(pid)
        pid = nil
        break if @cancel_thread
        sleep 1
      end
    ensure
      Process.kill('TERM', pid) rescue nil if pid
      Process.wait(pid) rescue nil if pid
    end
  end
end

OmsCosmosPlugin::OmsBridgeMicroservice.run if __FILE__ == $0
