require 'openc3/microservices/microservice'

module OmsCosmosPlugin
  class OmsBridgeMicroservice < OpenC3::Microservice
    def run
      env_val = ENV.fetch('OMS_BRIDGE_BINARY', '')
      bundled = File.join(__dir__, 'bin', 'oms_bridge')
      binary = (env_val.start_with?('/') ? env_val : bundled)
      File.chmod(0o755, binary) if File.exist?(binary) && !File.executable?(binary)

      cfg_dir = File.join(__dir__, 'config')
      ENV['RCAL_CALCONFIG_PATH'] = File.join(cfg_dir, 'CALConfig.toml') unless File.exist?(ENV['RCAL_CALCONFIG_PATH'].to_s)
      ENV['OMS_FIELD_MAPPINGS_PATH'] = File.join(cfg_dir, 'oms_field_mappings.toml') unless File.exist?(ENV['OMS_FIELD_MAPPINGS_PATH'].to_s)
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
