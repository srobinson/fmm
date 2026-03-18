require 'json'
require 'net/http'
require_relative 'config'
require_relative 'lib/helpers'

module Cacheable
  def cache_key
    "#{self.class.name}:#{id}"
  end

  def cached?
    !cache_key.nil?
  end
end

class DataProcessor
  include Comparable
  include Enumerable

  attr_accessor :name, :data
  attr_reader :status

  def initialize(name, data = [])
    @name = name
    @data = data
    @status = :pending
  end

  def process
    @data.reject(&:empty?).map(&:upcase)
  end

  def <=>(other)
    name <=> other.name
  end

  def each(&block)
    @data.each(&block)
  end

  private

  def validate
    raise ArgumentError, "Name required" if @name.nil?
  end
end

class ProcessConfig
  attr_accessor :max_retries, :timeout, :debug

  def initialize(max_retries: 3, timeout: 30, debug: false)
    @max_retries = max_retries
    @timeout = timeout
    @debug = debug
  end
end

def transform(input)
  input.map { |item| item.to_s.strip }.compact
end

def _internal_helper
  "private"
end
