require 'openc3/microservices/microservice'

module OmsCosmosPlugin
  class OmsBridgeMicroservice < OpenC3::Microservice
    def run
      binary = ENV.fetch('OMS_BRIDGE_BINARY', 'oms_bridge')
      loop do
        pid = spawn(ENV.to_h, binary)
        Process.wait(pid)
        break if @cancel_token.cancelled?
        sleep 1
      end
    end
  end
end

OmsCosmosPlugin::OmsBridgeMicroservice.run if __FILE__ == $0
