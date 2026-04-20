package com.example.actors

import akka.actor.Actor
import akka.actor.Props
import scala.collection.mutable

case class ProcessMessage(data: String)
case class ResultMessage(result: String)

trait MessageHandler {
  def handle(msg: Any): Unit
}

class DataActor extends Actor with MessageHandler {
  private val buffer = mutable.ListBuffer.empty[String]

  override def receive: Receive = {
    case ProcessMessage(data) => sender() ! ResultMessage(data.toUpperCase)
    case _ => ()
  }

  override def handle(msg: Any): Unit = ()
  private def cleanup(): Unit = buffer.clear()
}

object DataActor {
  def props: Props = Props(new DataActor)
  val MAX_BUFFER: Int = 1000
}
