package com.example.spark

import org.apache.spark.SparkConf
import org.apache.spark.sql.SparkSession
import org.apache.spark.sql.DataFrame

case class JobConfig(
  appName: String,
  master: String,
  inputPath: String,
  outputPath: String
)

object SparkJob {
  def main(args: Array[String]): Unit = {
    val config = JobConfig("MyJob", "local[*]", args(0), args(1))
    val spark = createSession(config)
    val df = loadData(spark, config.inputPath)
    val result = transformData(df)
    saveData(result, config.outputPath)
    spark.stop()
  }

  def createSession(config: JobConfig): SparkSession = {
    SparkSession.builder()
      .appName(config.appName)
      .master(config.master)
      .getOrCreate()
  }

  private def loadData(spark: SparkSession, path: String): DataFrame = {
    spark.read.parquet(path)
  }

  private def saveData(df: DataFrame, path: String): Unit = {
    df.write.parquet(path)
  }
}

def transformData(df: DataFrame): DataFrame = df

implicit val defaultConfig: JobConfig =
  JobConfig("default", "local", "/input", "/output")
