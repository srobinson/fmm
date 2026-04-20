
require 'logger'

module Loggable
  def log(message)
    logger.info(message)
  end

  def logger
    @logger ||= Logger.new($stdout)
  end
end

module Configurable
  def configure
    yield self if block_given?
  end
end

class Application
  include Loggable
  include Configurable
  extend Configurable

  def initialize
    @started = false
  end

  def start
    log("Starting application")
    @started = true
  end
end
