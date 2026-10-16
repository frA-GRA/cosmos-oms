require 'openc3/microservices/microservice'

module OmsCosmosPlugin
  class OmsBridgeMicroservice < OpenC3::Microservice
    def run
      default = File.join(__dir__, 'bin', 'oms_bridge')
      binary = ENV.fetch('OMS_BRIDGE_BINARY', '').then { |v| v.empty? ? default : v }
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
