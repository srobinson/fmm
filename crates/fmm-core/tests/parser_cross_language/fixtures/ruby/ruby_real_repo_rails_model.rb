
require 'json'
require_relative 'concerns/searchable'

module Searchable
  def search(query)
    # search logic
  end
end

class User
  include Searchable

  attr_accessor :name, :email
  attr_reader :id

  def initialize(name:, email:)
    @name = name
    @email = email
    @id = generate_id
  end

  def to_json
    JSON.generate({ name: @name, email: @email })
  end

  def valid?
    !@name.nil? && !@email.nil?
  end

  private

  def generate_id
    SecureRandom.uuid
  end
end

def create_user(name:, email:)
  User.new(name: name, email: email)
end
