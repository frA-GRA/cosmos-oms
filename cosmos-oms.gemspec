Gem::Specification.new do |s|
  s.name    = "cosmos-oms"
  s.version = "1.0.6"
  s.summary = "OMS UCI bridge plugin for OpenC3 COSMOS"
  s.description = "Bridges OMS UCI messages to OpenC3 COSMOS via a TCP interface and microservice."
  s.authors = ["OMS Bridge"]
  s.files   = Dir["plugin.txt", "targets/**/*", "microservices/OMS_BRIDGE/**/*"]
end
